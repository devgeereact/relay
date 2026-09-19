# Current Task Contract

## Outcome

Review Relay's stage display, timers, outputs and preacher phone/tablet connection against ProPresenter 7's documented behaviour. Produce an implementation plan and begin the local connection safeguards.

## Scope

- Review the existing implementation and recorded decisions.
- Produce `docs/superpowers/plans/2026-09-19-stage-timers-mobile.md` with findings, phases and acceptance checks.
- Repair unavailable mobile links and stage QR failure handling locally.
- Include overtime-capable Pause/Resume and operator-saved stage layouts in the plan, as requested by the user.
- Preserve unrelated work and congregation/rehearsal safety rules.

## Outside scope

- Cloud features, new authentication, NDI or native SDI.
- Remote synchronisation, publishing, merging or deployment.
- Claiming physical-phone or projector success from source or automated tests.

## Mode and workflow

- Primary mode: Existing Application.
- Workflow: Change Safety.

## Evidence

- Review findings tied to source symbols and official references.
- URL and clock arithmetic reproduction; regression tests checked against original code.
- Frontend/Rust tests, production web build, formatting and lint checks.
- Explicit separation of automated evidence and physical-device checks still required.

## Completion

The review and phased plan are written and the first safeguards are verified locally. The larger timer/layout upgrade remains planned. See the plan for evidence, remaining work and the next slice: address refresh and selecting the intended stage channel.
