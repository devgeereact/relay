#!/usr/bin/env bash
#
# Re-sign a locally built Relay.app with the HARDENED RUNTIME and the real
# entitlements — using an ad-hoc signature, so it needs no certificate and no
# Apple Developer account.
#
# ── WHAT THIS IS FOR ────────────────────────────────────────────────────────
#
# CLAUDE.md §17 / RELEASING.md: the microphone dies on the first correctly-signed
# build. Notarization requires the hardened runtime, and under it a process that
# opens an audio input device without `com.apple.security.device.audio-input` is
# killed by TCC — not politely refused. Without `NSMicrophoneUsageDescription` it
# is terminated the instant it asks.
#
# That trap used to be untestable until you owned a $99/year certificate, because
# `tauri build` without a signing identity produces a bundle with NO hardened
# runtime (and, in fact, no bundle signature at all — only a linker-signed binary,
# which is why `codesign --verify` reports the confusing "code has no resources but
# signature indicates they must be present").
#
# It is testable. `codesign --options runtime` turns the hardened runtime on for an
# ad-hoc signature exactly as it does for a Developer ID one, and TCC enforces
# entitlements the same way. So this script reproduces the §17 conditions on any
# Mac, for free:
#
#   npm run tauri build
#   ./scripts/sign-local.sh
#   open -a Relay          # then press Start Listening and confirm it hears you
#
# If the microphone works here, the entitlements are right. If Relay is killed the
# moment it listens, they are not — and you have found it before a church did.
#
# ── WHAT THIS IS NOT ────────────────────────────────────────────────────────
#
# This does NOT make Gatekeeper accept the app, and nothing can except a real
# `Developer ID Application` certificate plus notarization by Apple. `spctl` will
# still say "rejected", and a copy that has been downloaded (and so carries the
# quarantine flag) will still show "Relay is damaged and can't be opened".
#
# An ad-hoc signature has no identity behind it — that is the entire point of one.
# To actually ship to a church, follow docs/RELEASING.md §2.
#
set -euo pipefail

REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
APP="${1:-$REPO/src-tauri/target/release/bundle/macos/Relay.app}"
ENTITLEMENTS="$REPO/src-tauri/relay.entitlements"

if [ ! -d "$APP" ]; then
  echo "error: no app bundle at $APP" >&2
  echo "       run 'npm run tauri build' first, or pass a path." >&2
  exit 1
fi
if [ ! -f "$ENTITLEMENTS" ]; then
  echo "error: entitlements missing at $ENTITLEMENTS — see CLAUDE.md §17." >&2
  exit 1
fi

echo "signing (ad-hoc, hardened runtime): $APP"
codesign --force --sign - --options runtime --entitlements "$ENTITLEMENTS" "$APP"

# The verification below is the point of the script — a silent success proves
# nothing, so assert on the two things that actually differ from `tauri build`.
echo
echo "── verdict ──────────────────────────────────────────────────────────"
codesign --verify --strict "$APP" && echo "signature      : valid"

flags="$(codesign -d -v "$APP" 2>&1 | grep -oE 'flags=0x[0-9a-f]+\([^)]*\)' || true)"
echo "code flags     : ${flags:-unknown}"
case "$flags" in
  *runtime*) echo "hardened runtime: ON  — §17 conditions reproduced" ;;
  *) echo "hardened runtime: OFF — §17 is NOT being tested; signing failed to apply it" >&2; exit 1 ;;
esac

if codesign -d --entitlements - --xml "$APP" 2>/dev/null | grep -q "com.apple.security.device.audio-input"; then
  echo "mic entitlement : present"
else
  echo "mic entitlement : MISSING — the microphone will be killed by TCC (§17)" >&2
  exit 1
fi

if /usr/libexec/PlistBuddy -c "Print :NSMicrophoneUsageDescription" "$APP/Contents/Info.plist" >/dev/null 2>&1; then
  echo "usage string    : present"
else
  echo "usage string    : MISSING — macOS will terminate Relay the instant it asks (§17)" >&2
  exit 1
fi

echo
echo "Gatekeeper still rejects this build, and that is expected: an ad-hoc"
echo "signature carries no identity. Shipping needs docs/RELEASING.md §2."
