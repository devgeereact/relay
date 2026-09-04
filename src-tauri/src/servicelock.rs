//! SERVICE LOCK — what may not happen while a service is being recorded.
//!
//! Single responsibility: decide whether one named action is allowed to run right
//! now, and say why in words an operator can act on.
//!
//! ## Why this exists
//!
//! The console is a full editing environment and a live control surface at the same
//! time, on one screen, operated by a volunteer under time pressure with a room
//! watching. Settings, the Library and the Templates editor are all one click from
//! the transport. Nothing has ever stopped a mis-click from deleting the template
//! that is on the projector, or starting a 1.6 GB model download over the church's
//! broadband, in the middle of a sermon.
//!
//! So while a service is being recorded, a small set of actions is held back.
//!
//! ## The line this draws, and why it is drawn there
//!
//! Two things are protected, and only two:
//!
//! 1. **Irreversible.** Every `delete_*`. There is no undo in this product, and a
//!    deletion made by accident at 10:31 is gone.
//! 2. **Takes the engine away mid-sermon.** Swapping or downloading the speech
//!    model, a bulk import, changing the active Bible translation. Each stops or
//!    stalls the thing that is currently listening to a preacher.
//!
//! **Nothing on the fire path is protected, and nothing the operator uses to run a
//! service is protected.** Firing, nav, clear, blackout, rehearsal, sensitivity,
//! cue control, opening and closing outputs, assigning a screen's template — all
//! unaffected. That is not an oversight:
//!
//! * The panic controls must work at every moment, in every state, and a lock that
//!   could refuse one would be a lock that can hurt a congregation (DECISIONS §20).
//! * A template swap is deliberately live (DECISIONS §29) and is a repair tool: an
//!   unreadable verse on the wall is fixed by changing its look, during the service,
//!   which is exactly when it is discovered.
//! * Template EDITING is likewise left alone. It re-renders the wall, which is a
//!   real hazard — and it is also the only way to fix a template that is failing in
//!   front of people. Blocking the repair to prevent the risk is the wrong trade at
//!   10:31; the risk is visible on the operator's own preview and the repair is not
//!   available anywhere else.
//!
//! ## The operator can always override it
//!
//! "Operator override is a first-class control, never a fallback UI" (CLAUDE.md).
//! A lock the person in the room cannot lift would put this file above them, and
//! that is precisely backwards: it exists to catch an **accident**, not to overrule
//! a decision. One action unlocks it, it stays unlocked for the rest of that
//! service, and it re-arms on the next one.
//!
//! ## Growing this list is a decision, not a chore
//!
//! `PROTECTED` is enumerated rather than pattern-matched, every entry carries the
//! sentence an operator reads, and `tests` pins both that every name is a real
//! registered command and that no live-path command has crept in. Over-blocking is
//! the more dangerous failure here: an operator who cannot do the thing they need
//! at 10:31 has been harmed by the safety feature.

use crate::error;
use std::sync::atomic::{AtomicBool, Ordering};

/// Whether a recorded service is currently being protected.
///
/// Armed when a service starts, released when it ends, and liftable by the
/// operator at any point in between.
#[derive(Default)]
pub struct ServiceLock(AtomicBool);

impl ServiceLock {
    pub fn engaged(&self) -> bool {
        self.0.load(Ordering::Relaxed)
    }
    /// Arm the lock. Called when a service starts recording.
    pub fn arm(&self) {
        self.0.store(true, Ordering::Relaxed);
    }
    /// Release it — the service ended, or the operator lifted it deliberately.
    pub fn release(&self) {
        self.0.store(false, Ordering::Relaxed);
    }
    pub fn set(&self, on: bool) {
        self.0.store(on, Ordering::Relaxed);
    }

    /// May `command` run right now?
    ///
    /// Returns a `Refused` error — the typed kind that means *pressing it again
    /// will not help* (`error.rs`), because it genuinely will not until the service
    /// ends or the operator lifts the lock. The message names the action, says what
    /// is being protected, and says how to proceed; a refusal an operator cannot act
    /// on is just a dead button with extra steps.
    pub fn guard(&self, command: &str) -> error::Result<()> {
        if !self.engaged() {
            return Ok(());
        }
        let Some(what) = describe(command) else {
            return Ok(());
        };
        Err(error::Error::refused(format!(
            "A service is being recorded, so Relay is holding this back: {what}. \
             It can wait until the service ends — or unlock in Settings → Backup & Recovery \
             if you need to do it now."
        )))
    }
}

/// Every action held back during a service, with the phrase the operator reads.
///
/// Enumerated on purpose. A predicate over names (`starts_with("delete_")`) would
/// silently capture a future command nobody weighed, and this list has to stay
/// short: everything on it is something a volunteer might legitimately want.
pub const PROTECTED: &[(&str, &str)] = &[
    // ── Irreversible ────────────────────────────────────────────────────────
    ("delete_template", "deleting a template"),
    ("delete_channel", "removing a screen"),
    ("delete_plan", "deleting a service plan"),
    ("delete_song", "deleting a song"),
    ("delete_arrangement", "deleting an arrangement"),
    ("delete_saved_scripture", "deleting saved scripture"),
    ("delete_announcement", "deleting an announcement"),
    ("delete_media", "deleting a media file"),
    ("delete_voice_profile", "deleting a voice profile"),
    ("delete_service", "erasing a recorded service"),
    // ── Takes the engine away mid-sermon ────────────────────────────────────
    ("download_model", "downloading a speech model"),
    ("select_stt_model", "changing the speech model"),
    ("load_stt_model", "reloading the speech model"),
    (
        "install_model_file",
        "installing a speech model from a file",
    ),
    ("set_active_translation", "changing the Bible translation"),
    ("import_media", "importing media"),
    ("save_reviewed_songs", "saving an import"),
];

/// The phrase for a protected command, or `None` if it is not protected.
pub fn describe(command: &str) -> Option<&'static str> {
    PROTECTED
        .iter()
        .find(|(name, _)| *name == command)
        .map(|(_, what)| *what)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nothing_is_held_back_when_no_service_is_recording() {
        let lock = ServiceLock::default();
        assert!(!lock.engaged());
        for (cmd, _) in PROTECTED {
            assert!(
                lock.guard(cmd).is_ok(),
                "{cmd} must be free outside a service"
            );
        }
    }

    #[test]
    fn a_recorded_service_holds_back_exactly_the_protected_list() {
        let lock = ServiceLock::default();
        lock.arm();
        for (cmd, _) in PROTECTED {
            assert!(lock.guard(cmd).is_err(), "{cmd} must be held back");
        }
        assert!(lock.guard("something_new").is_ok(), "only the named list");
    }

    /// THE PANIC CONTROLS AND THE FIRE PATH MUST NEVER BE ON THIS LIST.
    ///
    /// A lock that can refuse `blackout` is a lock that can hurt a congregation,
    /// and one that can refuse `manual_fire` takes the override away at the exact
    /// moment the AI has got something wrong. Over-blocking is the more dangerous
    /// failure of the two this file can commit.
    #[test]
    fn the_lock_can_never_reach_the_live_path() {
        let live = [
            "manual_fire",
            "nav",
            "clear_screens",
            "blackout",
            "set_rehearsal",
            "confirm_detection",
            "dismiss_detection",
            "set_stage_next",
            "fire_content",
            "fire_media",
            "push_announcement",
            "start_countdown",
            "set_detection_enabled",
            "set_sensitivity",
            "set_thresholds",
            "open_channel_output",
            "close_channel_output",
            "set_channel_template",
            "save_template",
            "start_capture",
            "stop_capture",
            "end_service",
            "output_beat",
        ];
        let lock = ServiceLock::default();
        lock.arm();
        for cmd in live {
            assert!(
                lock.guard(cmd).is_ok(),
                "{cmd} is a live control and must never be held back by the service lock"
            );
            assert!(describe(cmd).is_none());
        }
    }

    #[test]
    fn the_operator_can_lift_it_and_the_next_service_re_arms_it() {
        let lock = ServiceLock::default();
        lock.arm();
        assert!(lock.guard("delete_template").is_err());
        // The person in the room decides. Nothing here outranks them.
        lock.release();
        assert!(lock.guard("delete_template").is_ok());
        // …and the next service protects itself again, so an override is scoped to
        // the service it was made in rather than quietly disarming Relay forever.
        lock.arm();
        assert!(lock.guard("delete_template").is_err());
    }

    #[test]
    fn every_refusal_names_the_action_and_says_how_to_proceed() {
        let lock = ServiceLock::default();
        lock.arm();
        for (cmd, what) in PROTECTED {
            let msg = lock.guard(cmd).unwrap_err().to_string();
            assert!(
                msg.contains(what),
                "{cmd}: the message must name the action"
            );
            assert!(
                msg.contains("unlock"),
                "{cmd}: a refusal an operator cannot act on is a dead button"
            );
        }
    }

    #[test]
    fn a_refusal_is_typed_refused_not_a_bare_string() {
        // `Refused` is the kind that means "pressing it again will not help", which
        // is exactly true here — and it is what stops the UI offering Try again.
        let lock = ServiceLock::default();
        lock.arm();
        let e = lock.guard("delete_media").unwrap_err();
        assert!(matches!(e, error::Error::Refused { .. }));
    }

    /// A NAME ON THE LIST WITH NO GUARD AT ITS CALL SITE IS A LIE.
    ///
    /// This list only means anything if every entry is actually enforced, and the
    /// failure is silent in the worst direction: the console shows "held back
    /// during a service", the operator believes it, and the command runs anyway.
    /// It is the same shape as the rehearsal leak — a rule stated in one place and
    /// skipped at one of its call sites — so it is checked by reading the source
    /// rather than trusted.
    #[test]
    fn every_protected_command_actually_guards_itself() {
        const MAIN: &str = include_str!("main.rs");
        for (name, _) in PROTECTED {
            assert!(
                MAIN.contains(&format!(r#"guard("{name}")"#)),
                "`{name}` is listed as protected but nothing calls lock.guard(\"{name}\") — \
                 the lock would report it as held back and let it run"
            );
            // …and it must be a real command, not a name that was renamed away.
            assert!(
                MAIN.contains(&format!("\n            {name},")),
                "`{name}` is not registered in generate_handler! — the list is stale"
            );
        }
    }

    /// THE GUARD MUST BE THE FIRST THING THE COMMAND DOES.
    ///
    /// A guard placed after the work has started refuses the operator without
    /// preventing anything, which is the worst of both: the side effect happens
    /// and they are told it did not.
    #[test]
    fn no_protected_command_does_work_before_it_checks() {
        const MAIN: &str = include_str!("main.rs");
        for (name, _) in PROTECTED {
            let call = format!(r#"guard("{name}")"#);
            let at = MAIN.find(&call).expect("checked above");
            // Walk back to the start of this function body and make sure nothing
            // between `{` and the guard touches the database or the filesystem.
            let head = &MAIN[..at];
            let body_start = head.rfind(") -> ").map(|i| i + 5).unwrap_or(0);
            let before = &head[body_start..];
            for forbidden in ["db.0.lock()", "std::fs::", "conn,"] {
                assert!(
                    !before.contains(forbidden),
                    "`{name}` reaches {forbidden} before its service-lock guard"
                );
            }
        }
    }

    #[test]
    fn the_list_has_no_duplicates_and_no_empty_phrases() {
        let mut names: Vec<&str> = PROTECTED.iter().map(|(n, _)| *n).collect();
        names.sort_unstable();
        let before = names.len();
        names.dedup();
        assert_eq!(
            before,
            names.len(),
            "a duplicate entry means two answers to one question"
        );
        for (name, what) in PROTECTED {
            assert!(!name.is_empty() && !what.is_empty());
        }
    }
}
