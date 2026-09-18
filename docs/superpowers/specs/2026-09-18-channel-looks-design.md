# Channel looks — a per-screen, per-kind template

**Date** 2026-09-18 · **Status** specified, not built · **Prerequisite** landed

---

## Why this is written down before it is built

The operator's report was that *"each screen should be able to have their own
template set to what they want"*. Investigating it found something underneath that
had to be fixed first, and was: **"Follow the content look" had never worked.** The
per-kind look was stored, the id crossed both doors, and nothing read it. A
following screen wore `default_template_id`, always. Worse, the Outputs tile
resolved the look correctly while idle, so the one surface an operator uses to
check the setup showed it working over a wall wearing something else.

That is fixed. This document is the feature that sits on top of it, and the reason
it is separate is that building it on a rung nobody had ever seen work would have
been building on sand.

---

## The gap that remains

`output_channels.template_id` gives a screen ONE template for ALL content kinds.
`app_settings['tpl_<kind>']` gives a per-kind default that is GLOBAL to every
screen. So an operator can have **per-kind globally**, or **per-screen uniformly**,
and never *"on the main screen scripture looks like this and songs look like that,
while the lobby TV uses something else for both."*

---

## The data model

A row per (screen, kind), not a column per kind. `CONTENT_KINDS` is already
mirrored by hand in three places with no test linking them; a column would make DDL
the fourth mirror and the least editable of them.

```sql
CREATE TABLE channel_looks (
    channel_id  INTEGER NOT NULL REFERENCES output_channels(id) ON DELETE CASCADE,
    kind        TEXT    NOT NULL,
    template_id INTEGER NOT NULL REFERENCES templates(id),
    PRIMARY KEY (channel_id, kind)
);
```

`template_id` is `NOT NULL` deliberately: **no row is the only way to say "this kind
inherits"**. An absent row and a NULL row would have to mean the same thing at every
reader, and two spellings of one fact is exactly what the role map already refuses.

`kind` carries no `CHECK`. SQLite cannot `ALTER` a `CHECK`, so a sixth content kind
would mean the table rebuild rule 25 is the scar of. A kind nobody reads is an inert
row; a rebuild before the window is shown is not.

### The migration, and the back-fill it refuses

`ensure_channel_looks` is one `CREATE TABLE IF NOT EXISTS`. Retryable by having
nothing to retry: no scratch table, no rebuild, no transaction opened, so rule 25's
failure mode has no way to arise.

**There is no back-fill, and refusing one is the whole of the migration.** An
install with four channels each carrying a `template_id` ends with zero rows, every
kind falls through to the screen's own template, and first launch is identical **by
construction** rather than by comparison.

The tempting back-fill — write each screen's current template into all five kinds so
the shape is explicit — changes nothing on day one and everything on day two: the
operator then changes that screen's template and four kinds silently keep the old
one, with no control having been touched.

### The fifth retirement door

`ensure_retired_presets_are_gone` deletes a retired preset only when nothing points
at it, checking four doors. `channel_looks` is a **fifth** and must be added in the
same commit, asserted separately from the other four. Missing it deletes a template
a screen is wearing for one kind, on a Sunday, silently. Two guards: the migration
goes on the ladder BEFORE the retirement, and a failed read of the fifth door
retires nothing.

---

## The resolution chain

A per-kind look **is not an override**. It is the screen's own template, for one
kind — so it joins one notch above the screen's blanket template, not at the
override level. That is what leaves DECISIONS §29 and the transparency law intact.

0. `templateShows`, unchanged, consulted on the SCREEN'S own template and never on
   the resolved per-kind look — otherwise choosing a look for a kind could turn that
   kind off, silently.
1. **The transparency law** — a keyed screen never accepts an opaque pinned override.
2. **A pinned cue template** — a deliberate choice about *this item* outranks a
   standing preference about *this screen*.
3. **The screen's look for this kind** — new.
4. **The screen's own template** — §29, unchanged.
5. **The global content look for this kind** — §70.
6. **The configured default.**
7. **`DEFAULT_TEMPLATE`**, the bundled floor.

The law is evaluated ONCE, against the pinned claimant only, after rungs 3 and 4
have produced "the screen's template":

```
screenLook = channel_looks[kind] ?? channel.template_id
if (screenLook == null) return pinned ?? contentLook ?? default ?? DEFAULT_TEMPLATE
if (pinned == null)     return screenLook
if (isKeyedTemplate(screenLook) && !isKeyedTemplate(pinned)) return screenLook
return pinned
```

The null branch comes first because `isKeyedTemplate(null)` is true, so a following
screen would otherwise "keep its keyed template", which is nothing.

**The law must not run between rungs 3 and 4.** The naive implementation calls the
existing resolver twice, screen-template as channel and per-kind look as override;
that applies the law to the per-kind look and silently discards an opaque
Announcement look on a lower-third screen — which is §29's original complaint
verbatim.

---

## What crosses the wire

**The content frame does not change at all.** Per-kind looks are configuration, and
configuration already has a working pattern in this hub: the role map. Copy it.

A new retained slot and a `channel_looks` frame carrying ids only, channels and
kinds with no row omitted, sent on every hello **including `{}`** — `{}` is an
answer, and a page that cannot tell "nobody has told me" from "I have no per-kind
look" paints the wrong template for one frame.

Hello ordering: with the configuration, before the retained screen frame. A look
must be in hand before the frame it dresses.

Published on change on **both doors** — a Tauri emit for the native window, which
has the bridge and no socket, and the hub frame for every browser source, which has
the socket and no backend.

The JSON behind the ids reuses what the content-look fix already built: the hello
reply sends a `template` frame for exactly the ids this install's map names, once
per connect, never per fire. **A default template still rides as an ID only** — the
13 MB PERF rule is untouched.

---

## The UI

**The screen card does not change.** A card is 232px of picture and two words;
twenty choices do not go there. Its `<select>` keeps its meaning and becomes the
screen's look **for everything else**.

One new control, in the inspector only: a disclosure called **"Per kind…"**.
Collapsed by default, summary reading **"Same look for every kind"**, zero rows.
Four screens times five kinds is twenty choices only for a church that asks for
twenty; for everyone else it is four choices and a closed line of text.

Each row's inherit option **names its destination, resolved live** — `Same as this
screen · Classic · Scripture`, or `Follow the content look · Aurora · Scripture`, or
`Follow the configured default · Classic Serif`. An inherit option that reads
identically whether a content look is set or not is not a line.

The summary names the kinds, never a count. The card gains one text term on its meta
line, no new lamp and no new colour.

`set_channel_look(channel_id, kind, template_id: Option<i64>)` — `None` deletes the
row. **Not service-lock protected**, for the reason `rename_channel` states: it is
reversible, and the moment an operator most wants it is when a look turns out wrong,
which is during a service.

---

## What this deliberately does not do

**Per-cue-per-screen targeting (RG-161) stays closed and this does not open it.** The
content frame is unchanged, the publish stays global and unrouted, and retention is
one configuration slot replayed to everyone. **It decides a LOOK at the receiver and
never a DESTINATION at the sender.**

The line to hold, and it will be pushed: **a per-kind look changes what a screen
wears, never whether it paints.** The moment somebody proposes "a look of NONE means
this screen skips this kind", that is RG-161 arriving through the back door with
none of its pieces and none of its tests. `layout.shows` is the standing answer to
that question and it lives on the template.

No per-kind background, transition or role. No back-fill. No `CHECK` on `kind`. No
look ids in the browser-source URL. No second resolver and no resolution in Rust —
the client resolves, because the client is the screen.

---

## Tests, in the order they must be watched to fail

1. `ensure_channel_looks_is_retryable` — three calls, one connection.
2. `an_upgraded_install_behaves_identically_on_first_launch` — fails the moment
   anyone adds the back-fill.
3. `a_template_a_screen_wears_for_one_kind_is_not_retired` — the fifth door,
   asserted separately.
4. `every_kind_this_module_publishes_has_an_explicit_verdict` — already exists, must
   be seen red before the new verdict is added.
5. `the_look_map_is_not_a_screen_frame`.
6. `a_client_that_connects_mid_service_is_sent_the_look_map_before_the_verse`.
7. `a_screen_wears_a_different_look_for_each_kind`, asserted on **both** doors.
8. `a_panic_control_takes_every_screen_whatever_look_it_was_wearing` — no channel and
   no kind anywhere on the clear/black path.
9. The pure resolver, rungs enumerated — **the per-kind-beating-blanket case must be
   watched to fail against the naive two-call implementation**, which applies the
   transparency law between rungs 3 and 4.
10. `templateShows` still reads the SCREEN'S template, never the resolved look.
11. The inspector and card previews resolve through the same call as the wall.
