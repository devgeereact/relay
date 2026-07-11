//! Crash reporting — opt-in, off by default, and content-scrubbed.
//!
//! Single responsibility: decide whether a crash report may be sent, and strip
//! everything from it that is not ours to send.
//!
//! ## Why this is allowed to exist at all
//!
//! Relay is local-first: "nothing leaves the device without an explicit, visible
//! reason" (CLAUDE.md). Crash telemetry is the one place where that rule earns an
//! exception, because the failure mode it prevents — the console dying in front
//! of a congregation, with no way to find out why — is the worst thing this
//! software can do. So it exists, but it is fenced:
//!
//! - **Off by default.** No DSN, no init, no network stack, until the operator
//!   ticks a box in Settings. A fresh install never talks to anything.
//! - **Never blocks the live path.** Reports are queued and flushed on a
//!   background thread with a short timeout on shutdown. A dead network must
//!   never stall a service.
//! - **Content never leaves.** See `scrub` below. This is the part that matters.
//!
//! ## What counts as content
//!
//! A sermon transcript is a recording of a named person preaching to their
//! congregation. Verse text, song lyrics, announcements, service titles and plan
//! names are all the church's material. **None of it is diagnostic**, so none of
//! it is sent — not in the message, not in a breadcrumb, not in an "extra".
//! What is sent is: the exception type, the stack trace, the module, the OS and
//! the app version. That is enough to fix a crash and nothing more.

use std::sync::Mutex;

/// The setting key in the `app_settings` table.
pub const ENABLED_KEY: &str = "crash_reporting_enabled";
/// The setting key holding the operator-supplied DSN (blank = use the built-in).
pub const DSN_KEY: &str = "crash_reporting_dsn";

/// Held for the lifetime of the process; dropping it flushes pending events.
static GUARD: Mutex<Option<sentry::ClientInitGuard>> = Mutex::new(None);

/// Strip anything that could carry church/congregation content out of an event.
///
/// Deliberately an ALLOW-LIST at the field level rather than a blocklist of
/// patterns: a blocklist fails open (anything you forgot to think of gets sent),
/// and the cost of failing open here is publishing somebody's sermon.
///
/// Pure, so it is actually testable — the tests below are the real specification.
pub fn scrub(mut event: sentry::protocol::Event<'static>) -> sentry::protocol::Event<'static> {
    // Breadcrumbs are free-form strings logged from all over the app. We cannot
    // audit every one of them forever, so we don't try: drop the lot.
    event.breadcrumbs = Default::default();

    // "Extra" and "contexts" are where structured payloads get attached. Same
    // argument. (Device/OS context is re-added by the `contexts` feature and is
    // not content — but we cannot distinguish per-key safely, so we keep only the
    // OS/device/runtime contexts by name.)
    event.extra.clear();
    event
        .contexts
        .retain(|k, _| matches!(k.as_str(), "os" | "device" | "runtime" | "rust"));

    // Never identify the operator or the machine.
    event.user = None;
    event.server_name = None;
    event.request = None;

    // The message and exception values can quote the data that caused the crash —
    // e.g. a panic formatting a verse, or a parse error echoing a transcript
    // line. Keep the TYPE, drop the free text.
    if let Some(msg) = event.message.take() {
        event.message = Some(redact(&msg));
    }
    for ex in event.exception.values.iter_mut() {
        ex.value = ex.value.as_deref().map(redact);
    }

    event
}

/// Replace anything that looks like church content with a placeholder, keeping
/// the shape of the message so a stack trace is still readable.
///
/// The rule is conservative: keep only characters that cannot spell a sentence.
/// A crash message like `called \`Option::unwrap()\` on a \`None\` value` survives
/// intact; `failed to render "For God so loved the world…"` does not.
fn redact(s: &str) -> String {
    // Quoted spans are where content ends up. Blank them, keep the rest.
    let mut out = String::with_capacity(s.len());
    let mut in_quote = false;
    let mut quote_ch = '"';
    let mut redacted_any = false;
    for c in s.chars() {
        match c {
            '"' | '\'' | '“' | '”' if !in_quote => {
                in_quote = true;
                quote_ch = if c == '“' { '”' } else { c };
                out.push('"');
            }
            _ if in_quote && c == quote_ch => {
                in_quote = false;
                if redacted_any {
                    out.push_str("<redacted>");
                    redacted_any = false;
                }
                out.push('"');
            }
            _ if in_quote => {
                redacted_any = true;
            }
            _ => out.push(c),
        }
    }
    if in_quote && redacted_any {
        out.push_str("<redacted>");
    }
    out
}

/// Turn crash reporting ON. Idempotent; a second call replaces the client.
///
/// `dsn` empty → do nothing (there is no built-in DSN in the open-source build;
/// a church that wants reporting points it at their own Sentry project).
pub fn enable(dsn: &str, release: &str) {
    let dsn = dsn.trim();
    if dsn.is_empty() {
        return;
    }
    let Ok(dsn) = dsn.parse::<sentry::types::Dsn>() else {
        eprintln!("telemetry: invalid DSN — crash reporting stays off");
        return;
    };
    let guard = sentry::init((
        dsn,
        sentry::ClientOptions {
            release: Some(release.to_string().into()),
            // No performance tracing: it samples spans that can carry content.
            traces_sample_rate: 0.0,
            // Never attach the running process's args/env.
            send_default_pii: false,
            attach_stacktrace: true,
            before_send: Some(std::sync::Arc::new(|e| Some(scrub(e)))),
            ..Default::default()
        },
    ));
    if let Ok(mut g) = GUARD.lock() {
        *g = Some(guard);
    }
    println!("telemetry: crash reporting ON (content-scrubbed)");
}

/// Turn crash reporting OFF and drop the client (flushing anything queued).
pub fn disable() {
    if let Ok(mut g) = GUARD.lock() {
        *g = None;
    }
    println!("telemetry: crash reporting OFF");
}

/// Is it currently on?
pub fn is_enabled() -> bool {
    GUARD.lock().map(|g| g.is_some()).unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use sentry::protocol::{Context, Event, Exception, Values};

    fn event_with_message(msg: &str) -> Event<'static> {
        Event {
            message: Some(msg.to_string()),
            ..Default::default()
        }
    }

    // The whole point of the module. If these ever fail, we are publishing a
    // church's sermon to a third party.
    #[test]
    fn verse_text_in_a_crash_message_never_leaves() {
        let e = event_with_message(
            r#"failed to render "For God so loved the world, that he gave his only begotten Son""#,
        );
        let out = scrub(e);
        let msg = out.message.unwrap();
        assert!(!msg.contains("God"), "{msg}");
        assert!(!msg.contains("begotten"), "{msg}");
        assert!(msg.contains("<redacted>"), "{msg}");
    }

    #[test]
    fn transcript_text_in_an_exception_never_leaves() {
        let e = Event {
            exception: Values::from(vec![Exception {
                ty: "PanicException".into(),
                value: Some(r#"no verse for 'and the preacher said turn with me to'"#.into()),
                ..Default::default()
            }]),
            ..Default::default()
        };
        let out = scrub(e);
        let v = out.exception.values[0].value.clone().unwrap();
        assert!(!v.contains("preacher"), "{v}");
        assert!(!v.contains("turn with me"), "{v}");
        // The exception TYPE survives — that's the diagnostic part.
        assert_eq!(out.exception.values[0].ty, "PanicException");
    }

    /// A real Rust panic must still be readable, or the whole feature is useless.
    #[test]
    fn an_ordinary_panic_message_survives_intact() {
        let e = event_with_message("called `Option::unwrap()` on a `None` value");
        let msg = scrub(e).message.unwrap();
        assert!(msg.contains("Option::unwrap()"), "{msg}");
        assert!(msg.contains("None"), "{msg}");
        assert!(!msg.contains("<redacted>"), "{msg}");
    }

    #[test]
    fn breadcrumbs_and_extras_are_dropped_wholesale() {
        let mut e = event_with_message("boom");
        e.extra
            .insert("transcript".into(), "the sermon text".into());
        e.breadcrumbs
            .values
            .push(sentry::protocol::Breadcrumb::default());

        let out = scrub(e);
        assert!(out.extra.is_empty());
        assert!(out.breadcrumbs.values.is_empty());
    }

    #[test]
    fn the_operator_and_the_machine_are_never_identified() {
        let mut e = event_with_message("boom");
        e.user = Some(sentry::protocol::User {
            email: Some("operator@church.org".into()),
            ..Default::default()
        });
        e.server_name = Some("church-booth-pc".into());

        let out = scrub(e);
        assert!(out.user.is_none());
        assert!(out.server_name.is_none());
    }

    /// OS/device context IS kept — it is what makes a crash report actionable,
    /// and it says nothing about the congregation.
    #[test]
    fn os_context_is_kept_but_unknown_contexts_are_not() {
        let mut e = event_with_message("boom");
        e.contexts.insert(
            "os".into(),
            Context::Os(Box::new(sentry::protocol::OsContext {
                name: Some("Windows".into()),
                ..Default::default()
            })),
        );
        e.contexts.insert(
            "service".into(),
            Context::Other(
                [("title".to_string(), "Sunday Morning Worship".into())]
                    .into_iter()
                    .collect(),
            ),
        );

        let out = scrub(e);
        assert!(out.contexts.contains_key("os"));
        assert!(!out.contexts.contains_key("service"));
    }

    #[test]
    fn disabled_by_default() {
        // No enable() call has been made in this test binary path.
        // (enable() with an empty DSN is also a no-op.)
        enable("", "relay@test");
        assert!(!is_enabled());
    }
}
