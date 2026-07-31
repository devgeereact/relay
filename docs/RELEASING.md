# Releasing Relay

Everything here is automated **except the parts that require your money and your
credentials.** Those are the two things I can't do for you, and there are exactly
two of them.

Once these are set up, releasing is: `git tag v0.2.0 && git push origin v0.2.0`.

---

## Why signing is not optional

An unsigned build is **stopped dead** by the operating system:

- **macOS** — *"Relay is damaged and can't be opened. You should move it to the Bin."* (That is what Gatekeeper says about an unsigned, un-notarized app. It is not damaged. The user has no way to know that.)
- **Windows** — SmartScreen: *"Windows protected your PC."*

A church volunteer does not push past those screens. They close the tab and go back to typing verses into PowerPoint. **This is the single biggest barrier between Relay and its first ten churches** — bigger than any missing feature.

---

## 1. The update signing key — do this first, and do it yourself

This keypair is what stops someone pushing malicious code to a church's machine
through the update endpoint. Relay will only ever install an update signed with
your private key.

```bash
npm run tauri signer generate -- -w ~/.relay/updater.key
```

It prints a **private key** and a **public key**.

> ### ⚠️ Never paste the private key into a chat, a terminal you're screen-sharing, an issue, or a commit.
> A private key that has been seen by anything other than you and GitHub's secret store is a compromised private key. If it ever leaks, anyone can sign an "update" and Relay will install it, silently, on every church that runs it. Regenerate immediately if that happens.

Then:

1. **Public key** → paste into `src-tauri/tauri.updater.conf.json` as `plugins.updater.pubkey`.
   **This is public and safe to commit** — and it is already committed, so you only do
   this if you are regenerating the key. (Regenerating it means no *existing* install can
   ever be updated again — they only trust the old key. Back the private key up instead.)
2. **Private key** → GitHub → Settings → Secrets and variables → Actions:
   - `TAURI_SIGNING_PRIVATE_KEY` — the private key's *contents*
   - `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` — the password you chose (empty string if none)

The release workflow **fails loudly** if the public key is still the placeholder.
That is deliberate: a release that silently produces unsigned artifacts is worse
than no release, because you find out only when a user can't open it.

---

## 2. macOS — Apple Developer ID (~$99/year)

You need a **Developer ID Application** certificate. Not a Mac App Store one —
Relay is distributed directly, not through the store.

1. Join the [Apple Developer Program](https://developer.apple.com/programs/) (~$99/yr).
2. In Xcode → Settings → Accounts → Manage Certificates → **+** → *Developer ID Application*.
3. Export it from Keychain Access as a `.p12` with a password.
4. Base64-encode it so it can live in a secret:
   ```bash
   base64 -i Certificates.p12 | pbcopy
   ```
5. Create an **app-specific password** at [appleid.apple.com](https://appleid.apple.com) → Sign-In and Security → App-Specific Passwords. (Notarization needs this; your real Apple password will not work.)

GitHub secrets:

| Secret | Value |
|---|---|
| `APPLE_CERTIFICATE` | the base64 blob from step 4 |
| `APPLE_CERTIFICATE_PASSWORD` | the `.p12` password |
| `APPLE_SIGNING_IDENTITY` | e.g. `Developer ID Application: Your Name (TEAMID)` |
| `APPLE_ID` | your Apple ID email |
| `APPLE_PASSWORD` | the **app-specific** password from step 5 |
| `APPLE_TEAM_ID` | your 10-character Team ID |

Notarization is Apple scanning the binary and stapling an approval to it. It takes
a few minutes and the workflow waits for it. **Without it, Gatekeeper still blocks
the app even if it is signed** — signing and notarizing are two separate steps and
you need both.

> ### Why the signed build is the one where the microphone works — and the unsigned one is a lie
>
> Notarization **requires the hardened runtime**, and Tauri enables it by default.
> Under the hardened runtime, opening an audio input device without the
> `com.apple.security.device.audio-input` entitlement is not "denied" — the process is
> killed by TCC. And without `NSMicrophoneUsageDescription`, macOS terminates the app
> the instant it *asks*.
>
> So before this was fixed, Relay behaved like this:
>
> | Build | Microphone |
> |---|---|
> | `tauri dev` | works (no hardened runtime) |
> | unsigned pre-release | works (ad-hoc signed, no hardened runtime) |
> | **signed + notarized** | **dead** |
>
> The first build correct enough to hand to a church would have been the first one
> that could not hear the preacher. `src-tauri/relay.entitlements` and
> `src-tauri/Info.plist` now carry both keys, and `models::config_boots` fails the
> build if either goes missing. **Do not "clean up" those files.**
>
> **You can reproduce these conditions locally, for free — you do not need the
> certificate.** The hardened runtime is not a property of *who* signed the app; it
> is a flag on the signature, and `codesign --options runtime` sets it for an ad-hoc
> signature exactly as for a Developer ID one. TCC then enforces entitlements the
> same way. So:
>
> ```bash
> npm run tauri build
> ./scripts/sign-local.sh          # ad-hoc + hardened runtime + the real entitlements
> open -a Relay                    # press Start Listening — it must hear you
> ```
>
> The script asserts the hardened runtime is actually ON, the mic entitlement is
> embedded, and the usage string survived into the bundled `Info.plist` — and exits
> non-zero if any of them is missing, so it cannot pass vacuously. This is the check
> that was missing when the table above was written; the row that says "no local
> build would have shown it" is no longer true, and that is the point.
>
> It still does **not** satisfy Gatekeeper. An ad-hoc signature carries no identity,
> so `spctl` says `rejected` and a downloaded copy still shows *"Relay is damaged"*.
> Only §2 above fixes that.

---

## 3. Windows — a code-signing certificate

Windows is messier than Apple, and the right choice depends on budget:

| Option | Cost | SmartScreen |
|---|---|---|
| **Azure Trusted Signing** | ~$10/month | Trusted quickly. **Recommended.** |
| **OV certificate** (Sectigo, DigiCert…) | ~$200–400/yr | Builds reputation slowly — early users still see warnings |
| **EV certificate** (hardware token) | ~$300–500/yr | Instant SmartScreen trust |
| **Unsigned** | free | **Blocked. Do not ship this.** |

For a free, open-source church tool, **Azure Trusted Signing** is almost certainly
the right answer: it's cheap, it's monthly, and it clears SmartScreen without a
hardware token in a drawer.

The workflow supports **either** scheme and picks between them by looking at which
secrets you set. You do not configure anything else.

**Azure Trusted Signing (recommended).** Create a Trusted Signing account and a
certificate profile in Azure, then register an app (service principal) and give it
the *Trusted Signing Certificate Profile Signer* role.

| Secret | Value |
|---|---|
| `AZURE_ENDPOINT` | e.g. `https://weu.codesigning.azure.net` |
| `AZURE_CODE_SIGNING_ACCOUNT` | your Trusted Signing account name |
| `AZURE_CERT_PROFILE` | the certificate profile name |
| `AZURE_CLIENT_ID` | service-principal app ID |
| `AZURE_CLIENT_SECRET` | service-principal secret |
| `AZURE_TENANT_ID` | your Azure tenant ID |

**Or a classic OV/EV `.pfx`:**

```bash
base64 -i certificate.pfx | pbcopy
```

| Secret | Value |
|---|---|
| `WINDOWS_CERTIFICATE` | the base64 `.pfx` blob |
| `WINDOWS_CERTIFICATE_PASSWORD` | the `.pfx` password |

> **Why signing is not in `tauri.conf.json`.** Tauri signs the Windows binaries
> *during* bundling — the `.exe` is signed before NSIS/WiX wraps it — so signing has
> to be configuration the bundler reads, not a post-build step. But a thumbprint or a
> `signCommand` committed to `tauri.conf.json` would break `npm run tauri build` for
> every contributor on Windows, because they don't hold the certificate. So the
> release workflow *generates* a `tauri.winsign.conf.json` for that build only and
> merges it over the base config with a second `--config` (the CLI merges configs in
> order). It is gitignored. Nothing about your certificate is ever committed.

---

## 4. Cut a release

```bash
npm run version:set -- 0.2.0          # writes all THREE version files
git commit -am "chore(release): 0.2.0"
git push
git tag v0.2.0
git push origin v0.2.0
```

> **Bump the version with the script, and commit it before you tag.** The version lives
> in three files — `src-tauri/tauri.conf.json`, `package.json`, `src-tauri/Cargo.toml` —
> and the one in `tauri.conf.json` is what the update manifest advertises. Tauri decides
> "is there an update?" by comparing that number, as semver, against what the church is
> running.
>
> It used to say `0.1.0` in all three, forever, and nothing in the release workflow ever
> read it. So a `v0.2.0` tag built the new binaries and published a `latest.json` that
> stamped them **version 0.1.0** — every existing install compared it to its own `0.1.0`,
> concluded it was already up to date, and never updated. No error, no warning, no
> symptom: just a fix that never arrives.
>
> The release gate now refuses to build if the tag and the repo disagree, and CI checks
> the three files agree on every PR. `npm run version:check` runs it yourself.

The workflow builds macOS (universal — one download for both Apple Silicon and
Intel) and Windows, signs each with that platform's certificate — and notarizes the
macOS build with Apple — then opens a **draft** release with `latest.json` attached.
**Draft, on purpose:** you look at it before a church does.

Publish the draft, and every existing install will offer the update on next launch.

### The gate is per-platform, and it will refuse the dangerous release

A plain version tag (`v0.2.0`) requires **both** certificates. If either platform is
unconfigured, the workflow fails before it builds anything and tells you exactly
which secrets are missing.

This is not paranoia — it is a bug we shipped. The gate used to test a single secret,
`APPLE_CERTIFICATE`, and call the whole release "signed". There was no Windows signing
config anywhere in the repo, so a real tag with the Apple secrets set produced a
correctly notarized `.dmg` **and an MSI that was never signed at all** — and the
"⚠️ unsigned build" warning in the release notes was keyed off the same single flag,
so it stayed silent too. Windows is the platform most of our churches are on.

An unsigned build is still allowed on a **pre-release** tag (one with a hyphen —
`v0.2.0-rc1`), because you need some way to exercise the pipeline before you own a
certificate. The release notes then say, per platform, which half is unsigned.

---

## What the operator sees

Relay checks for updates **once, on launch, and only when the microphone is off.**

> ### Relay never updates during a service.
> Not a dialog. Not a toast. Not a background download competing for a laptop's
> last 300 MB of RAM while whisper is running. An updater that interrupts a sermon
> is *worse than no updater* — it takes a tool that merely lacks a fix and turns it
> into a tool that actively causes a failure.
>
> `src/lib/updater.js` refuses to even *check* while capture is live, and refuses to
> *install* if capture starts. The banner says so out loud: *"Installing restarts the
> app, so do it before the service — not during."*

---

## Checklist

- [ ] `npm run tauri signer generate` run **by you**; private key in GitHub secrets, never anywhere else
- [ ] Public key pasted into `src-tauri/tauri.updater.conf.json` and committed
- [ ] Apple Developer Program joined; 6 Apple secrets set
- [ ] Windows certificate obtained; 2 Windows secrets set
- [ ] Tag pushed, draft release built
- [ ] **Downloaded the artifact on a machine that has never seen Relay, and opened it without a single OS warning**

That last one is the only test that counts.


## Building the DMG locally fails without `CI=1`

`npm run tauri build` bundles the `.app` fine and then dies:

```
failed to bundle project: error running bundle_dmg.sh
  ... execution error: Finder got an error: AppleEvent timed out. (-1712)
```

That is `bundle_dmg.sh` driving **Finder over AppleScript** to make the disk-image
window pretty (icon positions, background). It needs Automation permission to
control Finder, and it hangs on a machine that will not grant it.

It is cosmetic. Skip it:

```bash
CI=1 npm run tauri build
```

Tauri passes `--skip-jenkins` when `CI` is set, which skips the AppleScript and
produces a plain, perfectly functional DMG. **CI already sets `CI=true`, so the
release workflow is unaffected** — this bites only someone building on their own Mac,
where it looks like a broken release pipeline and is not one.

## Verifying that a release can actually UPDATE anyone

A release that installs fine and cannot update is the failure this whole feature
exists to prevent, and it is **completely silent**: the DMG works, the MSI works, the
app runs, and every existing installation is simply stranded on the version it first
installed, forever. Nothing errors. There is nothing in any log.

Two ways it happened here, both found by actually running the build:

1. **`bundle.targets` did not include `app`.** Tauri's macOS update bundle is
   `Relay.app.tar.gz`, and it is derived from the `app` target. With targets set to
   `["msi", "dmg"]`, `tauri build` cheerfully produced a DMG, exited 0 — and created
   **no update bundle and no signature at all**. Targets are now
   `["app", "dmg", "nsis", "msi"]`.

2. **`createUpdaterArtifacts` was `true` in the BASE config**, which meant any plain
   `tauri build` (a contributor's, CI's) died with *"a public key has been found, but
   no private key"*. It is now `false` in the base config and `true` only in the
   release overlay. Releases sign updates; nothing else should be trying to.

**Check it before you trust a tag.** With a throwaway key (never your real one):

```bash
npm run tauri signer generate -- -w /tmp/test.key -f --password ""
# put the .pub into a COPY of tauri.updater.conf.json, then:
TAURI_SIGNING_PRIVATE_KEY="$(cat /tmp/test.key)" \
TAURI_SIGNING_PRIVATE_KEY_PASSWORD="" \
CI=true npm run tauri build -- --config /tmp/updater.test.json

find src-tauri/target/release/bundle -name '*.sig' -o -name '*.tar.gz'
```

You must see **`Relay.app.tar.gz` AND `Relay.app.tar.gz.sig`** (and on Windows, the
`.nsis.zip` + `.sig`). If you see only a DMG or an MSI, the release cannot update
anyone. Delete the throwaway key afterwards.

> Note: `tauri build` warns that the identifier `com.relay.app` ends in `.app`.
> Leave it. It is cosmetic, and the identifier is the name of the app-data directory —
> changing it would orphan every existing church's database, service history and
> downloaded model.

## Unsigned pre-releases (before you have certificates)

Code-signing certificates take days to buy, and Windows SmartScreen reputation takes
*weeks* of downloads to earn. Waiting for them before ever exercising the release path
means the pipeline is first tested on the day it matters — which is how release
pipelines break.

So: **an unsigned build is allowed, but only on a pre-release tag.**

```bash
git tag v0.1.0-rc1 && git push origin v0.1.0-rc1      # unsigned, allowed
git tag v0.1.0     && git push origin v0.1.0          # FAILS without certificates
```

The rule is enforced in `release.yml`: a tag containing a hyphen is a pre-release and
may go unsigned; a plain version tag with no `APPLE_CERTIFICATE` fails loudly. **You
cannot ship an unsigned build to a church by accident** — you would have to type a tag
that says, in the tag itself, that it is not a real release.

An unsigned pre-release still gives you the two things worth testing:

- **real installers** (.dmg, .msi) built exactly as a real release builds them
- **a signed update bundle** (`.app.tar.gz` + `.sig`) — updater signing is a *different*
  key from OS code signing, and it is already configured

What it does not give you is the ability to hand it to a volunteer: macOS says *"Relay
is damaged and can't be opened"*, Windows SmartScreen warns. The release notes say so.

### What a pre-release does NOT give you: a live updater endpoint

This page used to claim the auto-updater "can be tested end to end today". **It cannot,
not from a pre-release**, and believing otherwise is how you find out the updater is
broken on the day you need it.

Relay's endpoint is:

```
https://github.com/devgeereact/relay/releases/latest/download/latest.json
```

GitHub's `/releases/latest/` resolves **only to a published, non-draft, non-prerelease
release**. So:

| Release | Served to installed apps? |
|---|---|
| draft (what the workflow opens) | **No** — assets aren't public until you publish |
| pre-release (`v0.2.0-rc1`) | **No** — `/latest/` skips prereleases, by design |
| published, plain tag (`v0.2.0`) | **Yes** |

That behaviour is *correct* — a church must never be auto-updated onto an unsigned
release candidate. But it means the endpoint stays dark until you publish a real
release, so the happy path is:

**tag `v0.2.0` → workflow opens a draft → you check it → you publish it → every install
offers the update on next launch.**

### How to actually test the updater before you own certificates

Build a local app that is *older* than the pre-release and point it at that
pre-release's manifest by its exact tag (not `/latest/`):

```bash
# 1. Cut an unsigned pre-release and let the workflow publish it (as a prerelease).
npm run version:set -- 0.2.0-rc1
git commit -am "chore(release): 0.2.0-rc1" && git push
git tag v0.2.0-rc1 && git push origin v0.2.0-rc1

# 2. Build a LOCAL app claiming to be 0.0.1, pointed at that tag's manifest.
#    The updater config comes first (it carries the pubkey); the override comes
#    second, because configs merge in order and the last value wins.
npm run tauri build -- \
  --config src-tauri/tauri.updater.conf.json \
  --config '{"version":"0.0.1","plugins":{"updater":{"endpoints":["https://github.com/devgeereact/relay/releases/download/v0.2.0-rc1/latest.json"]}}}'
```

Install that build, launch it, and it should offer 0.2.0-rc1. If it doesn't, the
updater is broken **now**, on your machine, where you can fix it — instead of on a tag
that a church is waiting on.
