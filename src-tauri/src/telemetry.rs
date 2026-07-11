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

    // Free text goes. All of it. See `REDACTED` below for why.
    if event.message.is_some() {
        event.message = Some(REDACTED.to_string());
    }
    for ex in event.exception.values.iter_mut() {
        // The exception TYPE stays — "PanicException", "std::io::Error". A type
        // name is code, not content, and it is what Sentry groups on.
        if ex.value.is_some() {
            ex.value = Some(REDACTED.to_string());
        }
        // The stack trace stays: frames are function names, files and line numbers
        // — code. But their captured LOCALS are not. A local variable at the moment
        // of a crash is very often the exact verse or transcript line that caused
        // it. Drop every one.
        if let Some(st) = ex.stacktrace.as_mut() {
            for frame in st.frames.iter_mut() {
                frame.vars.clear();
            }
        }
    }

    event
}

/// What replaces every free-text field in a crash report.
///
/// ## Why the message is dropped entirely, rather than cleaned
///
/// The first version tried to be clever: find the quoted spans (where content
/// usually ends up), blank those, and keep the rest so a panic stayed readable.
/// That is a **blocklist** — it enumerates what to remove and ships everything
/// else — and this module's own doc comment says, correctly, that a blocklist
/// fails open, and that the cost of failing open here is publishing somebody's
/// sermon.
///
/// It failed open immediately. An apostrophe is a quote character, and scripture
/// is full of them:
///
/// ```text
///   in:  no verse for 'God's word to the church'
///   out: no verse for "<redacted>"s word to the church"
///                                  ^^^^^^^^^^^^^^^^^^^ sent in the clear
/// ```
///
/// The `'` in `God's` closed the span early and the rest went out verbatim. The
/// tests passed, because I had tested the case I was thinking of and the leak was
/// in the case I wasn't.
///
/// There is no safe way to sift content out of a free-text field that is *allowed*
/// to contain content. So it isn't sifted — it's dropped. A crash stays fully
/// actionable from what remains: exception type, stack trace, module, OS, app
/// version. Which is exactly what this module always *claimed* it sent, and now
/// actually does.
const REDACTED: &str = "<redacted: Relay never sends message text>";

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
        assert_eq!(msg, REDACTED);
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

    /// THE test the original implementation would have failed.
    ///
    /// It tried to blank *quoted spans* and keep the rest. An apostrophe is a
    /// quote character, and scripture is full of them — so `God's` closed the span
    /// early and the remainder of the sermon text went out in the clear. The old
    /// tests passed because they used text without apostrophes.
    #[test]
    fn an_apostrophe_cannot_leak_the_rest_of_the_sentence() {
        for msg in [
            "no verse for 'God's word to the church'",
            "panic at 'thou shalt not' in Moses' law about coveting",
            "failed to render \"the Lord's prayer, our Father who art in heaven\"",
            "assertion failed: verse == The Lord is my shepherd, I shall not want",
        ] {
            let out = scrub(event_with_message(msg)).message.unwrap();
            for leaked in [
                "God", "church", "Moses", "shepherd", "Lord", "prayer", "shalt",
            ] {
                assert!(
                    !out.contains(leaked),
                    "leaked {leaked:?} from {msg:?} -> {out:?}"
                );
            }
        }
    }

    /// Nothing free-text survives, not even an innocuous-looking panic. A message
    /// that is ALLOWED to contain content cannot be sifted safely, so it is not
    /// sifted — the type and the stack trace are what make a crash actionable.
    #[test]
    fn no_free_text_survives_at_all() {
        let msg = scrub(event_with_message(
            "called `Option::unwrap()` on a `None` value",
        ))
        .message
        .unwrap();
        assert_eq!(msg, REDACTED);
    }

    /// Stack-frame LOCALS are dropped. A local at the moment of a crash is very
    /// often the exact verse that caused it.
    #[test]
    fn stack_frame_locals_are_dropped_but_the_frames_remain() {
        let mut frame = sentry::protocol::Frame {
            function: Some("relay::pipeline::fire".into()),
            lineno: Some(42),
            ..Default::default()
        };
        frame
            .vars
            .insert("verse".into(), "For God so loved the world".into());

        let e = Event {
            exception: Values::from(vec![Exception {
                ty: "PanicException".into(),
                value: Some("boom".into()),
                stacktrace: Some(sentry::protocol::Stacktrace {
                    frames: vec![frame],
                    ..Default::default()
                }),
                ..Default::default()
            }]),
            ..Default::default()
        };

        let out = scrub(e);
        let st = out.exception.values[0].stacktrace.as_ref().unwrap();
        // The frame survives — it is code, and it is what makes the report useful.
        assert_eq!(
            st.frames[0].function.as_deref(),
            Some("relay::pipeline::fire")
        );
        assert_eq!(st.frames[0].lineno, Some(42));
        // Its locals do not.
        assert!(
            st.frames[0].vars.is_empty(),
            "a local leaked the verse text"
        );
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
