<!--
Translating, or fixing a book name / numeral?

Delete this whole template and just say what you changed and which language you speak.
The checklist below is for code, and none of it applies to you. Thank you for doing the
most valuable work in this project.
-->

## What does this change, for the person in the booth?

<!-- One or two sentences. Not "refactored the router" — what is different for the
     volunteer running a service? If the answer is "nothing visible", say that; internal
     work is welcome and it just makes the review easier to know that up front. -->

## Why?

<!-- Link the issue if there is one. If this fixes a bug, describe how the bug FAILED —
     loudly, or silently? Relay's worst bugs have all been the silent kind. -->

---

## Checks

- [ ] `cargo fmt --all && cargo clippy --all-targets -- -D warnings` — clean
- [ ] `cargo test` and `npm test` — green
- [ ] I read [`docs/DECISIONS.md`](https://github.com/devgeereact/relay/blob/main/docs/DECISIONS.md). This does not contradict a recorded decision — or it does, and I have said so below and explained why the decision should change.

## If you fixed a bug

- [ ] **I reintroduced the bug and checked my test fails.**

      This is not ceremony. Several tests in this repo initially *passed on broken code*:
      a focus trap whose visibility check reported every element as hidden under jsdom,
      an entitlement test that grepped a code comment instead of the config. Both were
      caught only by putting the bug back. A test that cannot fail is not a test.

## If it runs during a service

<!-- The seven live modules: audio, dsp, stt, detection, router, pipeline, channels —
     plus anything in main.rs on the fire → nav → clear path. Delete this section if
     your change cannot execute while a congregation is watching. -->

- [ ] No `unwrap()` / `expect()` / panic path. A panic mid-sermon is the worst possible failure.
- [ ] No error is swallowed on a path the congregation can see. (The contract is at the top of `src/lib/stores/capture.js`: *can the congregation see the difference?*)
- [ ] **Nothing reports a success it did not achieve.** [`DECISIONS.md` §20](https://github.com/devgeereact/relay/blob/main/docs/DECISIONS.md) — a control that lies is worse than one that breaks.
- [ ] No `Mutex` is held across an `emit` / `broadcast_content`. (This deadlocked the macOS main loop, twice.)
- [ ] I did not borrow a colour that already carries a promise. **Amber = ON AIR. Amethyst = REHEARSAL.** A tally light that lies is worse than no tally light.
- [ ] `OutputContent` / `DetectionEvent` are built via `pipeline::Fire`, not by hand.

## If it touches the AI

- [ ] A paraphrase still cannot auto-fire — at **any** score, at **any** sensitivity. A TF-IDF cosine is not a probability, and the gate is the *method*, not the number.
- [ ] No number is shown to the operator that means something different depending on how it was produced.
