//! Hold the timers and answer questions about them. That is the whole job.
//!
//! DB-free, IO-free, and it does not know what a Tauri handle is — the same
//! discipline as `detection.rs` and `router.rs`. The clock is passed in as
//! `now_ms`, which is what makes every lifetime rule testable without a window.
//!
//! ## Why a registry rather than a slot
//!
//! The old `CountdownState` slot in `channels` was one `Mutex<Option<OutputContent>>`
//! filtered down to countdowns: the countdown
//! had no identity of its own, it WAS the live content, and so anything else put on
//! a screen forgot it. `adjust_countdown` then answered "Nothing is counting down."
//! with no way back. That is not a bug that can be patched where it lived — it is a
//! consequence of where the state was kept, so the state moved here.
//!
//! A timer has an id, a scope and a lifetime of its own. Nothing about a `Both`
//! timer's wire form changed when the registry arrived: it is still projected into
//! the `countdown_*` fields, which is what keeps the `is_countdown` guard in
//! `pipeline.rs` from refusing it as `Unsafe::Nothing`. The registry becomes the
//! source of truth and those fields become its projection. RG-149 added one field
//! to that projection and deliberately did not touch the guard — see
//! `BothProjection`, where the reasoning and its test are named.
//!
//! ## Lock discipline
//!
//! Innermost of all (rule 6): taken last, released first, and **never held across an
//! emit or a broadcast** (rule 2). Every method here takes the lock, finishes, and
//! returns owned values — there is no way to call this module and still be holding
//! it, which is the property that makes rule 2 unbreakable from the caller's side
//! rather than merely unbroken.
//!
//! A poisoned lock is recovered rather than propagated. The alternative during a
//! service is a timer that silently fails to exist: every operation here is one
//! short, self-contained map update, so there is no half-written state for a
//! previous panic to have left behind.

use std::collections::HashMap;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

pub type TimerId = i64;

/// Which screens a timer is about.
///
/// `Both` reaches every screen, through the content frame and the `countdown_*`
/// fields it projects into. `Stage` reaches the stage tablet only and
/// publishes no content frame at all — which is exactly why it survives a verse:
/// nothing about it rides on the live content, so replacing the live content cannot
/// forget it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Scope {
    Both,
    Stage,
}

/// One timer. `target_ms` and `from_ms` are epoch milliseconds.
///
/// `paused_ms` is the one exception to the deadline model, and it has to be: a held
/// timer is not an instant, it is a figure. `from_ms` is when it was aimed, so
/// `target_ms - from_ms` is the length it was aimed for and the warning rule has a
/// span to work from — it is never re-stamped by a re-aim.
///
/// `warn_ms` is the per-timer warning threshold — the figure somebody CHOSE for
/// this timer, in ms before zero. It is not read here (the rule is
/// `layers.js::countdownWarning`, one reading, on the far side of the bridge); it is
/// PROJECTED, by `project_both` for a congregation screen and by
/// `channels::timer_frame_json` for the stage. It had a carrier and no projection
/// for the whole of wave 3, which is RG-149(b): a `Both` timer could hold a chosen
/// threshold and no screen could be told about it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Timer {
    pub id: TimerId,
    pub label: String,
    pub done_msg: String,
    pub target_ms: i64,
    pub from_ms: i64,
    pub paused_ms: Option<i64>,
    pub warn_ms: Option<i64>,
    pub scope: Scope,
    pub plan_item_id: Option<i64>,
    /// WAS THIS TIMER STARTED INSIDE A REHEARSAL? — RG-150.
    ///
    /// Stamped once, by the creator, from the mode in force at that instant, and
    /// never written again. It is what `stop_started_in_rehearsal` reads at the
    /// rehearsal exit.
    ///
    /// **A property of the timer, never a question asked of a screen** — the same
    /// discipline `Scope` already carries for the panic controls
    /// (`channels::stop_congregation_timers`) and for the same reason: a control
    /// that has to ask a question can fail to answer it. Asking "is anything on
    /// the tablet that should not be" at the exit would be that control.
    ///
    /// `serde(default)` so an older payload, or a hand-built one, reads as a real
    /// service's timer. That is the safe absence: a timer wrongly kept is visible
    /// on a screen and can be stopped, and a timer wrongly taken is a clock that
    /// vanishes from the preacher's tablet mid-sermon with nothing to say why.
    #[serde(default)]
    pub started_in_rehearsal: bool,
}

/// Why an adjustment was refused. Both are refusals an operator can act on, not
/// faults — `main.rs` turns them into `error::Error::refused` sentences.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimerError {
    /// There is no timer with that id. Start is the only creator, so an adjustment
    /// that cannot find its timer refuses rather than quietly making a second one.
    NoSuchTimer,
    /// Less than a second would be left. The caller substitutes five minutes for a
    /// non-positive length, so a re-aim to zero would put 5:00 on a wall — the
    /// opposite of what was pressed.
    TooShort,
}

/// HOW LONG IS LEFT: the held figure when it is held, otherwise the gap to the
/// deadline, never below zero.
///
/// **The same rule as `countdown.js::countdownRemainingMs`.** One rule, stated twice
/// across the bridge because the bridge has two sides — and never a third time on
/// one side, which is the bug `docs/REBRAND.md` phase 7 records as fixed once
/// already.
pub fn remaining_ms(t: &Timer, now_ms: i64) -> i64 {
    t.paused_ms.unwrap_or(t.target_ms - now_ms).max(0)
}

/// What a `Both` timer looks like on the wire: the `countdown_*` fields and the
/// label that rides above them.
///
/// **THE FORM MOVED ONCE, FOR RG-149, AND THE RULE ABOVE IT DID NOT.** The
/// `is_countdown` guard in `pipeline.rs` recognises a countdown by
/// `countdown_to`/`countdown_paused_ms`/`kind == "countdown"`, and a timer with its
/// own field names and no text would be refused by the pre-air validator as
/// `Unsafe::Nothing` — a screen that stays blank while every log says the fire
/// succeeded. `countdown_warn_ms` is deliberately NOT a fourth recognition arm:
/// it says when to worry about a clock, it is not a clock, and widening the guard
/// to admit it would wave through a payload with a colour rule and nothing to
/// paint. That decision is pinned from the other side by
/// `pipeline::tests::a_warning_threshold_with_no_deadline_is_still_an_empty_screen`.
/// If this form moves again, `is_countdown` is re-read in the same commit.
pub struct BothProjection {
    pub reference: String,
    pub countdown_to: i64,
    pub countdown_from: i64,
    pub countdown_paused_ms: Option<i64>,
    pub countdown_done: String,
    /// The threshold somebody chose for THIS countdown, or None when nobody did.
    /// None is an absent figure and never a zero: a window of zero is a warning
    /// colour that never comes on, and the far side ranks it as absent
    /// (`layers.js::countdownWarning` — chosen, else configured, else the
    /// tenth-of-span rule).
    pub countdown_warn_ms: Option<i64>,
}

/// THE ONE PLACE THE CONGREGATION WIRE FIELDS ARE FILLED IN.
///
/// The registry owns the facts; these fields are its projection, not a second copy
/// that can drift from it. Two projectors is how five hand-rolled `OutputContent`
/// copies came to disagree, two of them silently dropping the scripture template.
pub fn project_both(t: &Timer) -> BothProjection {
    BothProjection {
        reference: t.label.clone(),
        countdown_to: t.target_ms,
        countdown_from: t.from_ms,
        countdown_paused_ms: t.paused_ms,
        countdown_done: t.done_msg.clone(),
        countdown_warn_ms: t.warn_ms,
    }
}

#[derive(Default)]
struct Inner {
    /// Monotonic, from 1, and never decremented — so an id is never handed out
    /// twice in the life of a registry. A reused id is a re-aim landing on the
    /// wrong clock: the operator presses `+1` on the timer they can see and moves
    /// one they cannot.
    next_id: TimerId,
    timers: HashMap<TimerId, Timer>,
}

/// The timers, and the answers to questions about them.
#[derive(Default)]
pub struct TimerRegistry(Mutex<Inner>);

impl TimerRegistry {
    /// The lock, with a poisoned one recovered rather than propagated — see the
    /// module doc. Not `unwrap()`: this runs during a live service.
    fn inner(&self) -> std::sync::MutexGuard<'_, Inner> {
        self.0.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Put a timer in the registry and hand back its identity. **The only creator.**
    /// The `id` field of the argument is ignored and replaced.
    pub fn start(&self, timer: Timer) -> TimerId {
        let mut g = self.inner();
        g.next_id += 1;
        let id = g.next_id;
        g.timers.insert(id, Timer { id, ..timer });
        id
    }

    /// The timer with that id, cloned, or None.
    pub fn get(&self, id: TimerId) -> Option<Timer> {
        self.inner().timers.get(&id).cloned()
    }

    /// Take one timer. True if there was one to take.
    pub fn stop(&self, id: TimerId) -> bool {
        self.inner().timers.remove(&id).is_some()
    }

    /// Take every timer in one scope and no other, reporting how many. A panic
    /// control asks for this by scope rather than asking each timer which screen it
    /// is on — a control that has to ask a question can fail to answer it.
    pub fn stop_scope(&self, scope: Scope) -> usize {
        let mut g = self.inner();
        let doomed: Vec<TimerId> = g
            .timers
            .values()
            .filter(|t| t.scope == scope)
            .map(|t| t.id)
            .collect();
        for id in &doomed {
            g.timers.remove(id);
        }
        doomed.len()
    }

    /// TAKE EVERY TIMER THAT WAS STARTED INSIDE A REHEARSAL, reporting how many —
    /// RG-150.
    ///
    /// A rehearsal is a sandbox in every other respect: nothing it publishes
    /// reaches a screen. A clock it started is not an exception to that, so ending
    /// the rehearsal ends them. The operator decision of 2026-09-17 records what
    /// the alternative cost: an operator who practises a twenty-minute sermon clock
    /// at ten o'clock would otherwise find it on the preacher's tablet when the
    /// service starts, counting toward a moment that has passed.
    ///
    /// **It reads the stamp and asks nothing else.** Not the scope, not what a
    /// screen is currently showing, not the clock — the same shape as `stop_scope`,
    /// which is why both can be stated in five lines and neither can be wrong about
    /// a timer it cannot see.
    ///
    /// ## What it deliberately leaves
    ///
    /// **A timer started BEFORE the rehearsal began survives it.** It was never a
    /// rehearsal's timer, so on the stated rule it is not one of these, and this
    /// takes only the stamped ones. The alternative reading — that a rehearsal exit
    /// clears everything — is a quiet widening, and it would take a real service's
    /// sermon clock off the preacher's tablet because somebody opened the rehearsal
    /// switch for ten seconds. Pinned by
    /// `e2e::a_timer_that_predates_a_rehearsal_survives_the_end_of_it`, on
    /// `Scope::Stage`, because leaving a rehearsal also clears the screens and a
    /// clear takes every congregation timer with it (DECISIONS §27) — an older
    /// guarantee, and not this one.
    pub fn stop_started_in_rehearsal(&self) -> usize {
        let mut g = self.inner();
        let doomed: Vec<TimerId> = g
            .timers
            .values()
            .filter(|t| t.started_in_rehearsal)
            .map(|t| t.id)
            .collect();
        for id in &doomed {
            g.timers.remove(id);
        }
        doomed.len()
    }

    /// RE-AIM OR HOLD A TIMER THAT IS ALREADY THERE. It can never create one.
    ///
    /// `remaining_ms` is how long should be left; `paused` whether it should be
    /// held. `None` for either means "leave that alone", so `+1` moves the time
    /// without touching the hold and Pause holds it without moving the time.
    ///
    /// Returns the timer as it now stands, so the caller never has to read it back
    /// — a second read is a second chance for the two to disagree.
    pub fn adjust(
        &self,
        id: TimerId,
        remaining_ms_arg: Option<i64>,
        paused: Option<bool>,
        now_ms: i64,
    ) -> Result<Timer, TimerError> {
        let mut g = self.inner();
        let t = g.timers.get_mut(&id).ok_or(TimerError::NoSuchTimer)?;

        let next = remaining_ms_arg.unwrap_or_else(|| remaining_ms(t, now_ms));
        if next < 1000 {
            return Err(TimerError::TooShort);
        }
        let hold = paused.unwrap_or(t.paused_ms.is_some());

        // `target_ms` stays set even while held: it is where the timer would land if
        // it were resumed now, and it is what keeps a projected `Both` timer reading
        // as a countdown to `preflight`, to the retained screen frame and to the
        // slide key.
        t.target_ms = now_ms + next;
        t.paused_ms = hold.then_some(next);
        // `from_ms` is NOT re-stamped — see `Timer`.
        Ok(t.clone())
    }

    /// Every timer, oldest first. The order is by identity, which is the order they
    /// were started in: a `HashMap` has no order at all, and a projection that picks
    /// "the newest" out of an unordered map picks a different one on different runs.
    pub fn snapshot(&self) -> Vec<Timer> {
        let g = self.inner();
        let mut all: Vec<Timer> = g.timers.values().cloned().collect();
        all.sort_by_key(|t| t.id);
        all
    }

    /// The same, narrowed to one scope.
    pub fn snapshot_scope(&self, scope: Scope) -> Vec<Timer> {
        let mut all = self.snapshot();
        all.retain(|t| t.scope == scope);
        all
    }

    /// WHICH TIMER BELONGS TO THIS CUE — and nothing at all when none does.
    ///
    /// `plan_item_id` was carried and never read: the field existed, `start_timer`
    /// wrote it, and no code anywhere could answer a question about it. This is
    /// its reader.
    ///
    /// The NEWEST wins, because ids only go up and a second timer for one cue can
    /// only mean the cue was put on air again. The clock the operator is looking at
    /// is the one that started last; answering with the older one would move a
    /// clock nobody is watching and leave the visible one running, which is the
    /// reused-id failure this registry already refuses to have.
    ///
    /// It returns `None` rather than the nearest thing it can find. A binding
    /// nobody has started is an ABSENCE, and a reader that guesses would re-aim
    /// some other cue's clock.
    pub fn for_plan_item(&self, plan_item_id: i64) -> Option<Timer> {
        let g = self.inner();
        g.timers
            .values()
            .filter(|t| t.plan_item_id == Some(plan_item_id))
            .max_by_key(|t| t.id)
            .cloned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A timer five minutes out, aimed at `now`.
    fn five(now: i64, scope: Scope) -> Timer {
        Timer {
            id: 0, // assigned by the registry; see `TimerRegistry::start`
            label: "Service begins in".into(),
            done_msg: "Welcome".into(),
            target_ms: now + 5 * 60_000,
            from_ms: now,
            paused_ms: None,
            warn_ms: None,
            scope,
            plan_item_id: None,
            started_in_rehearsal: false,
        }
    }

    /// A TIMER OUTLIVES CONTENT THAT IS NOT A TIMER.
    ///
    /// The reported defect in its smallest form. `CountdownState` forgot the
    /// countdown the moment the live content was anything else, so there was
    /// nowhere smaller than an end-to-end broadcast to pin the lifetime.
    ///
    /// The registry has no broadcast and no notion of content, which is exactly
    /// the point: there is no door here through which a verse could take the
    /// timer away. Only `stop`, `stop_scope` and a replacement may remove one.
    /// That the shipped doors stop forgetting is Task 3's assertion, not this
    /// one's.
    #[test]
    fn a_timer_outlives_content_that_is_not_a_timer() {
        let now = 1_000_000;
        let reg = TimerRegistry::default();
        let id = reg.start(five(now, Scope::Both));

        // Everything the rest of a service does to a registry, short of stopping
        // this timer: other timers arrive, and they are adjusted.
        let other = reg.start(five(now, Scope::Stage));
        reg.adjust(other, Some(90_000), None, now).expect("adjust");
        reg.stop(other);

        let still = reg.get(id).expect("the timer must still be there");
        assert_eq!(
            still.target_ms,
            now + 5 * 60_000,
            "the timer was forgotten, or quietly re-aimed, by traffic that was not about it"
        );
    }

    /// A HELD TIMER REPORTS THE FIGURE IT WAS HELD AT.
    ///
    /// A paused countdown is not an instant. `paused_ms` wins over the gap to the
    /// deadline at any `now_ms` — the same rule `countdown.js::countdownRemainingMs`
    /// keeps on the other side of the bridge.
    #[test]
    fn a_held_timer_reports_the_figure_it_was_held_at() {
        let now = 1_000_000;
        let mut t = five(now, Scope::Both);
        assert_eq!(
            remaining_ms(&t, now),
            5 * 60_000,
            "running: the gap to the deadline"
        );

        t.paused_ms = Some(90_000);
        for later in [now, now + 60_000, now + 10 * 60_000, now + 86_400_000] {
            assert_eq!(
                remaining_ms(&t, later),
                90_000,
                "a held timer moved with the clock at now_ms={later}"
            );
        }
    }

    /// A CUE'S CLOCK CAN BE FOUND, AND AN UNBOUND CUE FINDS NOTHING.
    ///
    /// `plan_item_id` rode out through `TimerView`'s flatten and nothing read it,
    /// so "does this cue already have a clock running" had no answer at all. The
    /// dangerous half is the second assertion: a reader that fell back to "the
    /// nearest timer" would hand back somebody else's clock, and the caller would
    /// re-aim or stop a timer the operator can see for a cue they are not on.
    #[test]
    fn a_cue_finds_its_own_timer_and_an_unbound_cue_finds_none() {
        let now = 1_000_000;
        let reg = TimerRegistry::default();

        assert_eq!(
            reg.for_plan_item(42),
            None,
            "an empty registry must answer with nothing, not with a guess"
        );

        // Two clocks that are NOT this cue's: one free-standing, one another cue's.
        reg.start(five(now, Scope::Stage));
        let other_cue = Timer {
            plan_item_id: Some(7),
            ..five(now, Scope::Stage)
        };
        reg.start(other_cue);
        assert_eq!(
            reg.for_plan_item(42),
            None,
            "a cue with no clock of its own must not be handed somebody else's"
        );

        let mine = reg.start(Timer {
            plan_item_id: Some(42),
            label: "Sermon".into(),
            ..five(now, Scope::Stage)
        });
        assert_eq!(reg.for_plan_item(42).map(|t| t.id), Some(mine));
        assert_eq!(
            reg.for_plan_item(7).map(|t| t.id),
            Some(mine - 1),
            "and the other cue still finds its own"
        );

        // Put the cue on air a second time. The clock somebody is looking at is
        // the one that started last.
        let again = reg.start(Timer {
            plan_item_id: Some(42),
            ..five(now, Scope::Stage)
        });
        assert_eq!(reg.for_plan_item(42).map(|t| t.id), Some(again));

        reg.stop(again);
        assert_eq!(
            reg.for_plan_item(42).map(|t| t.id),
            Some(mine),
            "stopping the newest falls back to the one still running, not to nothing"
        );
    }

    /// A RUNNING TIMER PAST ITS DEADLINE READS ZERO, NEVER A NEGATIVE.
    ///
    /// The clamp `adjust_countdown` already keeps. A negative would render as a
    /// count UP on a wall.
    #[test]
    fn a_timer_past_its_deadline_reads_zero() {
        let now = 1_000_000;
        let t = five(now, Scope::Both);
        assert_eq!(remaining_ms(&t, now + 10 * 60_000), 0);
    }

    /// A RE-AIM DOES NOT RELEASE A HOLD.
    ///
    /// **The one that costs a service if it is wrong.** `+1` on a countdown the
    /// operator deliberately stopped must not start it running in front of a
    /// congregation. `adjust_countdown`'s doc comment records this failure; this
    /// is the same rule, one layer down, where it can be stated in three lines.
    #[test]
    fn a_re_aim_does_not_release_a_hold() {
        let now = 1_000_000;
        let reg = TimerRegistry::default();
        let id = reg.start(five(now, Scope::Both));
        reg.adjust(id, None, Some(true), now).expect("hold it");

        let after = reg.adjust(id, Some(6 * 60_000), None, now).expect("+1");
        assert_eq!(
            after.paused_ms,
            Some(6 * 60_000),
            "a press of +1 released a timer the operator had stopped"
        );
        assert_eq!(
            remaining_ms(&after, now + 60_000),
            6 * 60_000,
            "and it must still be held a minute later"
        );
    }

    /// A RESUME RE-AIMS THE INSTANT FROM THE FIGURE IT WAS HOLDING.
    ///
    /// Releasing a hold must not teleport the deadline back to where the timer was
    /// originally aimed — it resumes from what was on the screen.
    #[test]
    fn a_resume_re_aims_the_instant_from_the_figure_it_was_holding() {
        let now = 1_000_000;
        let reg = TimerRegistry::default();
        let id = reg.start(five(now, Scope::Both));
        reg.adjust(id, None, Some(true), now).expect("hold");

        // Four minutes of real time pass while it is held.
        let later = now + 4 * 60_000;
        let running = reg.adjust(id, None, Some(false), later).expect("release");

        assert_eq!(
            running.paused_ms, None,
            "released, and still carrying the hold"
        );
        assert_eq!(
            running.target_ms,
            later + 5 * 60_000,
            "resume must put back what was HELD, not what was originally aimed at"
        );
        assert_eq!(remaining_ms(&running, later), 5 * 60_000);
    }

    /// A RE-AIM DOES NOT RE-STAMP WHEN THE TIMER WAS AIMED FROM.
    ///
    /// `from_ms` is the span the warning rule works from. Re-stamping it on every
    /// press would shrink the span to whatever is left, so the warning colour would
    /// come on at a different moment after each button press.
    #[test]
    fn a_re_aim_does_not_re_stamp_the_instant_it_was_aimed_from() {
        let now = 1_000_000;
        let reg = TimerRegistry::default();
        let id = reg.start(five(now, Scope::Both));
        let after = reg
            .adjust(id, Some(60_000), None, now + 30_000)
            .expect("re-aim");
        assert_eq!(
            after.from_ms, now,
            "the warning span moved under a button press"
        );
    }

    /// ADJUSTING A TIMER THAT IS NOT THERE IS REFUSED RATHER THAN CREATING ONE.
    ///
    /// Start is the one control that puts a timer in front of people, and there must
    /// be exactly one of those.
    #[test]
    fn adjusting_a_timer_that_is_not_there_is_refused_rather_than_creating_one() {
        let reg = TimerRegistry::default();
        let err = reg
            .adjust(42, Some(60_000), None, 1_000_000)
            .expect_err("there is no timer 42");
        assert_eq!(err, TimerError::NoSuchTimer);
        assert!(
            reg.snapshot().is_empty(),
            "a refusal created a timer — adjust is not a second creator"
        );
    }

    /// A TIMER SHORTER THAN A SECOND IS REFUSED.
    ///
    /// The rule `adjust_countdown` already keeps, moved down here with its reason:
    /// the caller substitutes five minutes for a non-positive length, so a re-aim to
    /// zero would put 5:00 on the wall — the opposite of what was pressed.
    #[test]
    fn a_timer_shorter_than_a_second_is_refused() {
        let now = 1_000_000;
        let reg = TimerRegistry::default();
        let id = reg.start(five(now, Scope::Both));

        for too_short in [0, 1, 999, -60_000] {
            assert_eq!(
                reg.adjust(id, Some(too_short), None, now),
                Err(TimerError::TooShort),
                "{too_short}ms was accepted"
            );
        }
        assert_eq!(
            reg.get(id).expect("still there").target_ms,
            now + 5 * 60_000,
            "a refused re-aim moved the timer anyway"
        );
        // …and a second is fine, so the boundary is where it says it is.
        reg.adjust(id, Some(1000), None, now).expect("one second");
    }

    /// TWO TIMERS HAVE TWO IDENTITIES.
    ///
    /// The whole point of the wave. Ids are distinct and `stop` takes one without
    /// touching the other.
    #[test]
    fn two_timers_have_two_identities() {
        let now = 1_000_000;
        let reg = TimerRegistry::default();
        let a = reg.start(five(now, Scope::Both));
        let b = reg.start(five(now, Scope::Stage));
        assert_ne!(a, b, "two timers were handed the same identity");

        reg.adjust(b, Some(60_000), None, now).expect("re-aim b");
        assert_eq!(
            reg.get(a).expect("a").target_ms,
            now + 5 * 60_000,
            "re-aiming one timer moved the other"
        );

        assert!(reg.stop(a), "stop reports whether it took one");
        assert!(reg.get(a).is_none());
        assert!(reg.get(b).is_some(), "stopping one timer took the other");
        assert!(
            !reg.stop(a),
            "stopping a timer twice must not claim a second one"
        );
    }

    /// AN IDENTITY IS NEVER HANDED OUT TWICE IN ONE RUN.
    ///
    /// A reused id is a re-aim landing on the wrong clock: the operator presses `+1`
    /// on the timer they can see and moves one they cannot.
    #[test]
    fn an_identity_is_never_reused_after_the_timer_is_stopped() {
        let now = 1_000_000;
        let reg = TimerRegistry::default();
        let first = reg.start(five(now, Scope::Both));
        reg.stop(first);
        let second = reg.start(five(now, Scope::Both));
        assert_ne!(
            first, second,
            "a stopped timer's id was handed to its successor"
        );
    }

    /// STOPPING A SCOPE TAKES EVERY TIMER IN IT AND NO OTHER.
    ///
    /// Track C's panic-control behaviour depends on this being a property of the
    /// registry rather than a branch inside `clear` or `black`. A panic control that
    /// has to ask which screen it is talking to can fail to answer.
    #[test]
    fn stopping_a_scope_takes_every_timer_in_it_and_no_other() {
        let now = 1_000_000;
        let reg = TimerRegistry::default();
        let congregation = [
            reg.start(five(now, Scope::Both)),
            reg.start(five(now, Scope::Both)),
        ];
        let programme = reg.start(five(now, Scope::Stage));

        assert_eq!(
            reg.stop_scope(Scope::Both),
            2,
            "it must report what it took"
        );
        for id in congregation {
            assert!(reg.get(id).is_none(), "a congregation timer survived");
        }
        assert!(
            reg.get(programme).is_some(),
            "clearing the congregation took the programme timer with it"
        );
        assert_eq!(
            reg.snapshot_scope(Scope::Stage).len(),
            1,
            "and the scope snapshot must agree"
        );
        assert!(reg.snapshot_scope(Scope::Both).is_empty());

        assert_eq!(
            reg.stop_scope(Scope::Both),
            0,
            "an empty scope takes nothing"
        );
    }

    /// A SNAPSHOT IS ORDERED OLDEST FIRST, SO "THE NEWEST" IS ANSWERABLE.
    ///
    /// `start_countdown` projects the newest `Both` timer into the four wire fields.
    /// A `HashMap` has no order at all, so without this the projection would pick a
    /// different timer on different runs — which is the shape of a bug that passes
    /// every test and puts the wrong clock on a wall once a month.
    #[test]
    fn a_snapshot_is_ordered_oldest_first() {
        let now = 1_000_000;
        let reg = TimerRegistry::default();
        let ids: Vec<_> = (0..8).map(|_| reg.start(five(now, Scope::Both))).collect();
        assert_eq!(
            reg.snapshot().iter().map(|t| t.id).collect::<Vec<_>>(),
            ids,
            "the snapshot must be ordered by identity, oldest first"
        );
        assert_eq!(
            reg.snapshot_scope(Scope::Both).last().map(|t| t.id),
            ids.last().copied()
        );
    }

    /// ENDING A REHEARSAL TAKES THE TIMERS IT STARTED AND NO OTHERS — RG-150.
    ///
    /// The registry half of the operator decision of 2026-09-17, where it can be
    /// stated without a window: the stamp decides, and nothing else is consulted.
    /// Both directions are asserted, because the two ways to be wrong here have
    /// very different costs — a rehearsal's clock left counting on a preacher's
    /// tablet, and a real service's clock taken off it.
    #[test]
    fn ending_a_rehearsal_takes_the_timers_it_started_and_no_others() {
        let now = 1_000_000;
        let reg = TimerRegistry::default();

        let real = reg.start(five(now, Scope::Stage));
        let rehearsed = [
            reg.start(Timer {
                started_in_rehearsal: true,
                ..five(now, Scope::Stage)
            }),
            // A congregation timer started in a rehearsal is one of these too. The
            // exit happens to clear the screens as well, which takes every `Both`
            // timer — but that is DECISIONS §27's guarantee and this one may not
            // lean on it, or the rule would hold only where something else already
            // held it.
            reg.start(Timer {
                started_in_rehearsal: true,
                ..five(now, Scope::Both)
            }),
        ];

        assert_eq!(
            reg.stop_started_in_rehearsal(),
            2,
            "it must report what it took"
        );
        for id in rehearsed {
            assert!(reg.get(id).is_none(), "a rehearsal's timer outlived it");
        }
        assert!(
            reg.get(real).is_some(),
            "the rehearsal exit took a timer that predates it — the widening this \
             decision deliberately does not make"
        );
        assert_eq!(
            reg.stop_started_in_rehearsal(),
            0,
            "a second exit takes nothing"
        );
    }

    /// A THRESHOLD CHOSEN FOR ONE TIMER SURVIVES THE ONE PROJECTION — RG-149(b).
    ///
    /// `warn_ms` was carried on the timer and dropped at the wire, so a `Both`
    /// timer could hold a chosen threshold and a congregation's screen could not be
    /// told about it: `project_both` had no warn field at all. That is the half of
    /// RG-149 no frontend test could see, because there was nothing on the wire for
    /// a frontend to read. The projection is the ONE place the congregation wire
    /// form is filled in (rule 36), so it is the one place the field can be lost.
    #[test]
    fn a_chosen_threshold_survives_the_one_projection() {
        let now = 1_000_000;
        let mut t = five(now, Scope::Both);
        t.warn_ms = Some(120_000);
        assert_eq!(
            project_both(&t).countdown_warn_ms,
            Some(120_000),
            "the congregation wire form cannot express a threshold anybody chose"
        );
        // An ABSENT threshold stays absent. A zero here would be a warning window
        // that never opens — a colour that never comes on, which is the same
        // reasoning `layers.js::setCountdownWarnDefault` keeps on its own side.
        assert_eq!(
            project_both(&five(now, Scope::Both)).countdown_warn_ms,
            None
        );
    }
}
