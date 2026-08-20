//! R5 · Failure & Boundaries — audit evidence.
//!
//! Test-only, and deliberately its own module rather than more tests inside
//! `qa.rs`: these are the REPRODUCTIONS behind three findings, and two of them
//! are expected to be RED until the findings are closed. Mixing a red-on-purpose
//! test into the shared harness makes `cargo test` unreadable for everyone else.
//!
//! Fixture and assertion surfaces come from [`crate::qa`] — one fixture, no drift.

use super::qa::{bare_app, settle};
use super::*;

// ── R5 · FAILURE & BOUNDARIES — audit evidence ──────────────────────────
//
// The three tests below were written by the failure-and-boundaries audit as
// EVIDENCE for findings, not as fixes. Two of them FAIL on purpose: they are
// the reproduction, and they should stay red until the finding is closed.

/// R5-2 · The LAN remote's route surface, pinned.
///
/// `docs/DECISIONS.md` owns the no-auth call, and the audit brief's job is not
/// to relitigate it — it is to notice the day the surface grows. `remote_api`
/// is a `match` with a `_ =>` arm, so a new route is one line and no test
/// anywhere counts them.
///
/// Seven routes. If this fails, someone widened an unauthenticated control
/// plane and the decision record has to be re-read before it merges.
#[test]
fn the_lan_remote_answers_exactly_seven_routes_and_refuses_the_rest() {
    let app = bare_app();
    let h = app.handle().clone();

    for route in [
        "search?q=john",
        "fire?ref=John 3:16",
        "next",
        "prev",
        "clear",
        "black",
        "live",
    ] {
        let body = super::remote_api(&h, super::remote_verb(route), route).body;
        assert!(
            !body.contains(r#""error":"unknown""#),
            "route {route:?} is part of the decided surface and stopped answering: {body}"
        );
    }

    // Anything else is refused. These are the shapes a new route would take.
    for route in [
        "",
        "status",
        "plan",
        "rehearsal",
        "settings",
        "shutdown",
        "detect",
        "announce",
        "media",
        "template",
        "service",
        "transcript",
        "history",
    ] {
        let body = super::remote_api(&h, super::remote_verb(route), route).body;
        assert!(
            body.contains(r#""error":"unknown""#),
            "the unauthenticated LAN control plane has grown a {route:?} route. \
             Re-read docs/DECISIONS.md §35 before this merges — it owns the no-auth \
             call and the exact size of the surface it applies to. Body: {body}"
        );
    }
}

/// R5-3 · `scrub` is an allow-list at the FIELD level and a BLOCKLIST at the
/// EVENT level — and its own doc comment says why that is the wrong shape.
///
/// telemetry.rs: *"Deliberately an ALLOW-LIST at the field level rather than a
/// blocklist of patterns: a blocklist fails open (anything you forgot to think
/// of gets sent), and the cost of failing open here is publishing somebody's
/// sermon."*
///
/// `scrub` enumerates the carriers it empties — breadcrumbs, extra, contexts,
/// user, server_name, request, message, exception values, exception frame
/// locals — and ships every other field of `Event` verbatim. Three of those
/// remaining fields are free-text and reachable from ordinary sentry APIs:
///
///   * `logentry` — what the `log` / `tracing` integrations write, and what
///     `capture_message` with formatting produces.
///   * `tags` — `sentry::configure_scope(|s| s.set_tag(…))`.
///   * `threads[].stacktrace.frames[].vars` — the SAME locals the exception
///     path is careful to clear, on the other stacktrace carrier.
///
/// Today nothing in Relay writes any of them: the only integration enabled is
/// `panic`, which fills `exception`. So this is not a live leak — it is the
/// guard rail being one `set_tag` away from not existing. THIS TEST FAILS,
/// deliberately, and closing it means turning `scrub` into what its comment
/// already claims: build a fresh `Event`, copy the allowed fields across.
#[test]
fn nothing_a_sentry_event_can_carry_survives_scrub() {
    use sentry::protocol::{Event, LogEntry, Stacktrace, Thread, Values};

    let verse = "For God so loved the world";
    let transcript = "turn with me to the book of Romans";

    let mut e: Event<'static> = Event {
        logentry: Some(LogEntry {
            message: format!("failed to render {verse}"),
            params: vec![transcript.into()],
        }),
        ..Default::default()
    };
    e.tags.insert("service".into(), "Sunday Morning".into());
    e.tags.insert("last_verse".into(), verse.into());

    let mut frame = sentry::protocol::Frame {
        function: Some("relay::pipeline::fire".into()),
        ..Default::default()
    };
    frame.vars.insert("verse".into(), verse.into());
    e.threads = Values::from(vec![Thread {
        stacktrace: Some(Stacktrace {
            frames: vec![frame],
            ..Default::default()
        }),
        ..Default::default()
    }]);

    let out = telemetry::scrub(e);
    let wire = serde_json::to_string(&out).expect("serialisable");

    for leaked in [verse, transcript, "Sunday Morning"] {
        assert!(
            !wire.contains(leaked),
            "a crash report carried church content off the device via a field \
             `scrub` does not enumerate.\n  leaked: {leaked:?}\n  wire: {wire}"
        );
    }
}

/// R5-4 · Two `next`s that overlap lose one press, because `handle_nav`
/// chooses its target under the Context lock and then RELEASES it before
/// `fire_manual` commits the advance.
///
/// Three independent threads reach `handle_nav`: the `nav` command (console
/// `→`), the STT worker (a spoken "next"), and the :8032 HTTP task (the
/// preacher's phone). Nothing serialises them, and the read-choose-commit is
/// not atomic — so two overlapping steps can both read verse N, both fire
/// N+1, and leave the cursor on N+1.
///
/// This test drives them SEQUENTIALLY, which is the honest thing a headless
/// harness can do: it establishes the correct sequential behaviour (N+1 then
/// N+2) so that the concurrency claim is stated against a measured baseline
/// rather than a guess. The interleaving itself is SUSPECTED and needs either
/// a lock held across choose-and-commit or a stress harness to prove.
#[test]
fn sequential_navs_step_one_verse_each_the_baseline_the_race_would_break() {
    let app = bare_app();
    let h = app.handle().clone();

    let _ = super::remote_api(&h, "POST", "fire?ref=Psalms%2023:1");
    settle();

    let a: serde_json::Value =
        serde_json::from_str(&super::remote_api(&h, "POST", "next").body).unwrap();
    let b: serde_json::Value =
        serde_json::from_str(&super::remote_api(&h, "POST", "next").body).unwrap();

    assert_eq!(a["nav"]["reference"], "Psalms 23:2", "first step: {a}");
    assert_eq!(
        b["nav"]["reference"], "Psalms 23:3",
        "second step must not repeat the first — if it ever does, the \
         choose-then-commit window in handle_nav has widened: {b}"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// R5-6 · THE DRIVE-BY — closed 2026-08-20.
//
// `<img src="http://<relay>:8032/api/black">` on any page, opened by anyone on the
// church network while browsing anything, used to black out the congregation's
// wall. Three individually reasonable choices composed into it: every action was a
// side-effecting GET, the request line was parsed verb-agnostically, and every
// response carried `Access-Control-Allow-Origin: *`. Nobody chose the result.
//
// DECISIONS §35 named the fix and left it undone — "if we do only one thing, do
// this". This is that one thing. It is NOT authentication: the LAN control plane
// stays deliberately open, because the preacher driving their own reading from a
// phone is the feature. What closes is the drive-by, whose audience is not "someone
// on the church wifi" but "anyone whose browser can be pointed at a URL".
// ═══════════════════════════════════════════════════════════════════════════════

/// A `GET` may not change what a congregation is looking at.
///
/// An `<img>`, a `<script>`, a stylesheet, a prefetch and a plain link can issue
/// nothing but `GET`, so refusing it removes the entire class — without a password
/// on a device three volunteers share.
#[test]
fn a_get_cannot_change_what_the_congregation_sees() {
    let app = bare_app();
    let h = app.handle().clone();

    // Precondition: a verse really is on the wall, so "nothing happened" below is a
    // statement about the gate and not about an empty room.
    let up = super::remote_api(&h, "POST", "fire?ref=John%203:16");
    assert_eq!(up.status, 200, "precondition: the POST path still works");
    let live: serde_json::Value =
        serde_json::from_str(&super::remote_api(&h, "GET", "live").body).unwrap();
    assert_eq!(live["live"]["reference"], "John 3:16", "precondition");

    for route in ["black", "clear", "next", "prev", "fire?ref=Psalms%2023:1"] {
        let reply = super::remote_api(&h, "GET", route);
        assert_eq!(
            reply.status, 405,
            "GET /api/{route} was honoured — the <img> drive-by is back (DECISIONS §35)"
        );
        assert!(
            !reply.cors,
            "GET /api/{route} was refused but still offered the CORS wildcard, so a \
             cross-origin page can read the refusal and probe the surface"
        );
        assert!(
            reply.body.contains("POST"),
            "the refusal must say what to do instead, got: {}",
            reply.body
        );
    }

    // ...and the wall is untouched by all of it.
    let after: serde_json::Value =
        serde_json::from_str(&super::remote_api(&h, "GET", "live").body).unwrap();
    assert_eq!(
        after["live"]["reference"], "John 3:16",
        "a GET moved the wall"
    );
}

/// The preacher's phone keeps working. This is the half a security fix usually
/// breaks, and the reason DECISIONS §35 chose this repair over a password.
#[test]
fn the_preachers_phone_still_drives_the_wall_over_post() {
    let app = bare_app();
    let h = app.handle().clone();

    let fired = super::remote_api(&h, "POST", "fire?ref=John%203:16-17");
    assert_eq!(fired.status, 200);
    assert!(fired.body.contains("\"ok\":true"), "{}", fired.body);

    let stepped: serde_json::Value =
        serde_json::from_str(&super::remote_api(&h, "POST", "next").body).unwrap();
    assert_eq!(stepped["nav"]["kind"], "fired");
    assert_eq!(stepped["live"]["reference"], "John 3:17");

    assert_eq!(super::remote_api(&h, "POST", "clear").status, 200);
    assert_eq!(super::remote_api(&h, "POST", "black").status, 200);
}

/// The READ-ONLY routes are unchanged, and deliberately so: they mutate nothing,
/// and a kiosk fetching them cross-origin is a real use. Narrowing them would be a
/// different decision than the one §35 records.
#[test]
fn the_read_only_routes_keep_working_over_get_and_keep_their_cors() {
    let app = bare_app();
    let h = app.handle().clone();

    for route in ["search?q=john", "live"] {
        let reply = super::remote_api(&h, "GET", route);
        assert_eq!(reply.status, 200, "GET /api/{route} stopped answering");
        assert!(
            reply.cors,
            "GET /api/{route} lost its CORS wildcard — a kiosk fetch depends on it, \
             and this route changes nothing"
        );
    }
}

/// A mutating route withholds the wildcard even when it SUCCEEDS.
///
/// Refusing the GET is the vector; withholding the wildcard is what stops a
/// cross-origin caller reading what a successful POST did to the wall.
#[test]
fn a_successful_mutation_is_not_readable_cross_origin() {
    let app = bare_app();
    let h = app.handle().clone();

    for route in ["fire?ref=John%203:16", "next", "clear", "black"] {
        let reply = super::remote_api(&h, "POST", route);
        assert_eq!(reply.status, 200, "precondition for {route}");
        assert!(
            !reply.cors,
            "POST /api/{route} succeeded and handed out the CORS wildcard"
        );
    }
}
