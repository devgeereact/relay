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
- Batch 6: RG-196 to RG-205, the audit's tier two; FIELD-2026-09-20 audit; document refresh and consolidation.

## Outside scope

- F13 (a live countdown transport on Live): removed on the operator's instruction on 2026-09-20; not rebuilt without a new instruction.
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
