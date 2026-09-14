/**
 * WHAT A SETTINGS ROW SAYS WHEN IT HAS NOTHING TO SAY.
 *
 * A list row is a name and a value. Several rows in Settings printed an em dash
 * instead — the LAN address, the installed version, the recognition language —
 * and an em dash is the same glyph for three different situations:
 *
 *   - the value has not been fetched yet;
 *   - the fetch failed;
 *   - there genuinely is no value (no network, no model, no answer).
 *
 * That is the defect RG-83 filed against the update banner in another costume: a
 * status that reads the same when everything is fine as when something is
 * broken is not a status (CLAUDE.md rule 35). An operator looking at "—" beside
 * *This machine* cannot tell whether the church has no network or whether Relay
 * failed to ask.
 *
 * So a row with no value says which of the three it is, in words, and the
 * absent-case wording belongs to the CALL SITE — only it knows what absence
 * means for that particular fact.
 */

/** Shown while a value is still being fetched. One phrase, everywhere. */
export const CHECKING = 'checking…';

/**
 * The text for one settings row.
 *
 * @param value    what was fetched (any type; `0` and `false` are values)
 * @param options  `loading` — the fetch has not finished;
 *                 `missing` — what absence MEANS here, as a short phrase
 */
export function settingValue(value, { loading = false, missing = 'unavailable' } = {}) {
  if (loading) return CHECKING;
  if (value === null || value === undefined) return missing;
  // A number is a value, including zero — `0 ms` and `0 dropped` are answers,
  // and `||` would have turned both into "unavailable".
  if (typeof value === 'number') return Number.isFinite(value) ? String(value) : missing;
  if (typeof value === 'boolean') return String(value);
  const s = String(value).trim();
  return s === '' ? missing : s;
}
