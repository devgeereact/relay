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
npm run tauri signer generate -- -w ~/.tauri/relay.key
```

It prints a **private key** and a **public key**.

> ### ⚠️ Never paste the private key into a chat, a terminal you're screen-sharing, an issue, or a commit.
> A private key that has been seen by anything other than you and GitHub's secret store is a compromised private key. If it ever leaks, anyone can sign an "update" and Relay will install it, silently, on every church that runs it. Regenerate immediately if that happens.

Then:

1. **Public key** → paste into `src-tauri/tauri.updater.conf.json`, replacing
   `PASTE_YOUR_TAURI_UPDATER_PUBLIC_KEY_HERE`. **This is public and safe to commit.**
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

Set `WINDOWS_CERTIFICATE` (base64 `.pfx`) and `WINDOWS_CERTIFICATE_PASSWORD`, or
switch the workflow to Azure's signing action — the step is already stubbed.

---

## 4. Cut a release

```bash
# bump the version in src-tauri/tauri.conf.json and package.json first
git tag v0.2.0
git push origin v0.2.0
```

The workflow builds macOS (universal — one download for both Apple Silicon and
Intel) and Windows, signs and notarizes both, and opens a **draft** release with
`latest.json` attached. **Draft, on purpose:** you look at it before a church does.

Publish the draft, and every existing install will offer the update on next launch.

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
