# Current Task Contract

## Outcome

Bring Relay to a state the operator can run a live service on with confidence: Phase 1 audit
(read-only, 2026-09-21, thirteen agents, findings cross-checked) followed by Phase 2 fixes in
ordered batches, each verified by running, each with its documents updated in the same commit.

## Scope

- Branch `phase2-service-readiness`, stacked on `stage-timers-mobile` (PR #92).
- Batch 1: RG-179 memory-resolved bare verse offered never fired; RG-180 auto-fire leaves the plan; RG-181 context keys under a dialog. Done.
- Batch 2: RG-182 media failure on the beat; RG-183 loop default; RG-184 codec probe. Done.
- Batch 3: RG-185 staging tags; RG-186 staged deck steps; RG-187 same-host media; RG-188 Turn on never over the console; RG-189 test fire armed. Done.
- Batch 4: RG-190 readiness runs every probe; RG-191 dead LAN server on every workspace; RG-192 claim column order and colour. Done.
- Batch 5: RG-193 output page staleness; RG-194 wall countdown on the host clock; RG-195 lagging client re-synced and counted. Done.
- Batch 6: RG-196 to RG-205, the audit's tier two; FIELD-2026-09-20 audit; document refresh and consolidation Done.
- Batch 7 (on the operator's "go ahead", 2026-09-21): RG-206 countdown transport restored (§109); RG-50 a second Bible, the BSB (§110); RG-207 the caution ink (§111); the V1 audit and launch checklist folded into RELAY_GAP §24/§27 and QA_HARNESS Parts 5/6 and archived. Done.
- Batch 8 ("go ahead with what's remaining", 2026-09-21): RG-208 timers persist and a relaunch brings them back (§112); RG-209 every build carries a marker and every service records it (S13); RG-50 option two, a licensed Bible imported from a file and deleted in two presses (§113). Done.

## Outside scope

- Transcoding (parked, needs a decoder Relay does not ship); the phone rehearsal (phase 6, needs a device); the elapsed timer (deferred with a reason, plan phase 3).
- Cloud features, new authentication, NDI or native SDI; transcoding.
- Remote synchronisation, publishing, merging or deployment.
- Claiming physical-phone or projector success from source or automated tests.

## Mode and workflow

- Primary mode: Existing Application.
- Workflow: Change Safety.

## Evidence

- Every fix has a test watched to fail first; e2e tests through the real commands where the fire path is touched.
- Both suites, clippy, fmt and the web build green at every commit; counts in the commit messages.
- Physical screens, phones and projectors: NOT TESTED.

## Completion

Batches 1 to 5 landed; batch 6 in progress. The stacked PR is opened when batch 6 lands, against `stage-timers-mobile`.
