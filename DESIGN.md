---
name: GRP Launcher
description: A blue-black cross-com visor for a community Ghost Recon Phantoms server; thin teal lines, one cyan signal, chamfered plates.
colors:
  ground: "#091015"
  deep: "#060b0f"
  plate: "#0f1a21"
  line: "#1f3b46"
  line-hi: "#3b7384"
  ink: "#e4eef1"
  ink-2: "#a3c0c8"
  ink-3: "#7397a1"
  signal: "#72e0ea"
  signal-hover: "#a2f0f6"
  signal-pressed: "#5cc9d3"
  signal-ink: "#04171b"
  fresh: "#f0b655"
  fault: "#ff7a66"
typography:
  brand:
    fontFamily: "Barlow Condensed, Bahnschrift, Segoe UI, system-ui, sans-serif"
    fontSize: "19px"
    fontWeight: 600
    lineHeight: 1
    letterSpacing: "0.14em"
  heading:
    fontFamily: "Barlow Condensed, Bahnschrift, Segoe UI, system-ui, sans-serif"
    fontSize: "17px"
    fontWeight: 600
    lineHeight: 1.1
    letterSpacing: "0.12em"
  play:
    fontFamily: "Barlow Condensed, Bahnschrift, Segoe UI, system-ui, sans-serif"
    fontSize: "23px"
    fontWeight: 600
    lineHeight: 1
    letterSpacing: "0.2em"
  label-caps:
    fontFamily: "Barlow Condensed, Bahnschrift, Segoe UI, system-ui, sans-serif"
    fontSize: "14px"
    fontWeight: 600
    lineHeight: 1
    letterSpacing: "0.1em"
  body:
    fontFamily: "Barlow Semi Condensed, Bahnschrift, Segoe UI, system-ui, sans-serif"
    fontSize: "15px"
    fontWeight: 400
    lineHeight: 1.4
  field-label:
    fontFamily: "Barlow Semi Condensed, Bahnschrift, Segoe UI, system-ui, sans-serif"
    fontSize: "14px"
    fontWeight: 400
    lineHeight: 1.4
  hint:
    fontFamily: "Barlow Semi Condensed, Bahnschrift, Segoe UI, system-ui, sans-serif"
    fontSize: "13.5px"
    fontWeight: 400
    lineHeight: 1.35
rounded:
  none: "0"
spacing:
  xs: "6px"
  sm: "10px"
  md: "16px"
  lg: "20px"
  chamfer: "9px"
components:
  button-primary:
    backgroundColor: "{colors.signal}"
    textColor: "{colors.signal-ink}"
    typography: "{typography.play}"
    rounded: "{rounded.none}"
    height: "54px"
  button-primary-hover:
    backgroundColor: "{colors.signal-hover}"
  button-primary-active:
    backgroundColor: "{colors.signal-pressed}"
  button-primary-disabled:
    backgroundColor: "{colors.line}"
    textColor: "{colors.ink-2}"
  button-ghost:
    backgroundColor: "transparent"
    textColor: "{colors.ink}"
    typography: "{typography.label-caps}"
    rounded: "{rounded.none}"
    height: "36px"
    padding: "0 14px"
  input:
    backgroundColor: "{colors.deep}"
    textColor: "{colors.ink}"
    typography: "{typography.body}"
    rounded: "{rounded.none}"
    height: "36px"
    padding: "0 10px"
  mode-switch-selected:
    backgroundColor: "{colors.line}"
    textColor: "{colors.ink}"
    typography: "{typography.label-caps}"
    height: "34px"
  operator-column:
    backgroundColor: "{colors.plate}"
    width: "340px"
    padding: "18px 20px 14px"
---

# Design System: GRP Launcher

## Overview

**Creative North Star: "The Cross-Com Visor"**

The window reads as the game's own targeting visor, not a store page. A blue-black ground carries thin teal hairlines and
line readouts; labels attach to what they describe through leader lines. One cyan signal marks what is live, chosen or
focused. Amber marks what is new. Red-orange marks what failed. Nothing else carries color. It is dark by design, not by
theme: there is one scheme (`color-scheme: dark`), no light variant, and no toggle.

The window is fixed at 1000 x 620 and not resizable (Tauri 2). Density is that of an instrument panel: 13.5 to 15px text,
36px controls, hairline dividers, almost no padding on the outside edge of plates. Surfaces are flat and square. The only
cut corners are the chamfer on the play plate; the only drawn imagery is the line-scanned terrain and the view-cone
diagram, both generated in `main.js` as SVG.

The identity is the project's own. It uses no Ubisoft artwork, logotype or key art, and it must not look official
(PRODUCT.md, owner decision of 23-nov-2026).

**Key Characteristics:**
- Blue-black ground, teal hairlines, one cyan signal; amber and red-orange only for state.
- Square everything (`border-radius: 0`); the 9px chamfer belongs to the play plate only.
- Two Barlow cuts: Barlow Condensed SemiBold for uppercase tracked labels, Barlow Semi Condensed for reading text.
- Tabular, lining figures everywhere (`font-variant-numeric: tabular-nums lining-nums`).
- Flat: no shadows, no blur, no glow, no glass. Depth is hairlines and one lighter plate.
- Motion is rare and spent in one place: the terrain scan and server tag on the home view.

## Colors

A restrained dark palette: five blue-black neutrals tinted toward teal, three text tones tinted from the same hue, one
cyan signal, and two state colors. Hex is the source of truth (frontmatter); values below name roles only.

### Primary
- **Cross-Com Cyan** (`signal`): live, chosen or focused. The play plate fill, the selected tab underline, the selected
  account mode underline, focus rings, checked checkboxes, range thumb and the filled part of the progress gauge, caret
  and text selection, the lit terrain lines, the server tag, the view-cone edge, the success status line. Hover and
  pressed on the play plate use `signal-hover` and `signal-pressed` (two fixed values, not derived).
- **Signal Ink** (`signal-ink`): text and check mark on a cyan fill only.

### Secondary
- **New Amber** (`fresh`): only for "new since your last visit": the "N new" count in the notes header and the "New"
  label on an unseen note.
- **Fault Coral** (`fault`): only for a failure message in the status line (`#status.fault`).

### Neutral
- **Night Ground** (`ground`): window background and the terrain's occluding fill.
- **Well** (`deep`): the lowest level, a notch darker than the ground: input, select and checkbox wells, the progress
  track, range track.
- **Plate** (`plate`): the one raised surface: the operator column, the sensitivity calculator box, select option lists.
- **Hairline** (`line`): dividers, resting input borders, the selected state of segmented switches (as a fill).
- **Hairline Lit** (`line-hi`): hover borders, ghost button borders, checkbox borders, scrollbar thumbs, terrain
  lines, tick marks.
- **Ink** (`ink`, #e4eef1): primary text, values, active tab.
- **Ink 2** (`ink-2`): field labels, secondary text, note bullets.
- **Ink 3** (`ink-3`): hints, captions, placeholders, inactive tabs and disabled text (5.5:1 on the ground; do not go
  dimmer).

### Named Rules
**The One Signal Rule.** Cyan means live, chosen or focused. It is never decoration, never a second brand color, and it
does not tint backgrounds except as the 8 to 18% wash on ghost hover/active and the 13% view-cone wedge.

**The State Colors Rule.** Amber means new, coral means failed, and each appears for those meanings only. A new state
needs a new meaning, not a new hue.

**The Hue Family Rule.** Every neutral and every text tone stays in the teal-blue family (hue about 190 to 200). Never
introduce pure grey or pure white.

## Typography

**Label Font:** Barlow Condensed SemiBold (600), bundled at `ui/fonts/`, fallbacks Bahnschrift, Segoe UI, system-ui.
**Body Font:** Barlow Semi Condensed Regular (400), Medium (500) and SemiBold (600), same fallbacks.
Both are Barlow (SIL OFL; license in `third-party/Barlow-OFL.txt`), loaded with `font-display: block` from local files
only. No font or script is fetched at run time (PRODUCT.md).

**Character:** Condensed capitals for anything that behaves like an instrument label; a slightly wider condensed text
face for sentences. The condensed width lets Spanish strings fit the 340px column and the 1000px window.

### Hierarchy
- **Brand** (600, 19px, 1, tracking .14em, uppercase, Condensed): the single h1 in the top bar.
- **Heading** (600, 17px, 1.1, tracking .12em, uppercase, Condensed): section titles (notes title, settings legends,
  the server tag name, reset title).
- **Play** (600, 23px, 1, tracking .2em, uppercase, Condensed): the play plate; 17px with .12em tracking when the label is
  long (`.primary.long`, `#resetGo`, create-account mode).
- **Label caps** (600, 13 to 15px, tracking .06 to .12em, uppercase, Condensed): tabs (15px), ghost buttons and the
  account mode switch (14px), language switch and "N new" (13px), the progress figure (15px) and the FOV readout (17px).
- **Body** (400, 15px, 1.4, Semi Condensed): base text, checkbox labels, note titles (600, 15px/1.35).
- **Field label / secondary** (400, 14px): form labels, note bullets (max 68ch), status line (14px/1.35), links.
- **Hint** (400, 13.5px, 1.35, `ink-3`, max 70ch): explanations under controls. Smallest sentence text is 13px (note
  meta); the smallest caps are 12px (note tag and "New" labels).

### Named Rules
**The Two Faces Rule.** Uppercase tracked text is Barlow Condensed; sentence-case text is Barlow Semi Condensed. Do not
set sentences in caps and do not set labels in the text face.

**The Tabular Figures Rule.** Figures are tabular and lining by default on `body`; progress, FOV, FPS and resolution
values must keep that so columns do not shift while they change.

## Layout

An `.app` grid of three rows: top bar 48px, main area `1fr`, bottom rail 34px. The main area is two columns: a flexible
field on the left and a fixed 340px operator column on the right, separated by a 1px hairline.

- **Field (left)** switches between two panels through the top-bar tabs (`data-view` on `#app`): Home and Settings.
  Home stacks a 214px terrain visor (with a hairline underneath) and the release notes, which scroll. Settings is one
  scrolling column of `fieldset.group`s, each a two-column grid with 14px row and 20px column gaps, hairline between
  groups, group title as a `legend`.
- **Operator column (right)** is always present on both views: account form (or password reset form, swapped in place),
  the play plate pinned to the bottom of the form with `margin-top: auto`, and a status block of fixed 78px height (the
  gauge, then the status text) beneath it. The play button stays one glance away on every view.
- **Bottom rail** carries the links (status, source, support) at the left and the legal line plus version at the right.
- **Rhythm:** outer padding 16 to 20px; label above control 5px; 11px between form fields; 36px control height; 34 to
  36px segmented switches; 54px play plate. Spacing is a handful of fixed pixel values, not a token scale; the
  frontmatter lists the ones that recur.
- **Responsive:** none. The window is fixed. Do not add breakpoints; design for 1000 x 620, with scroll regions
  (`overflow-y: auto`, thin scrollbar in `line-hi`) for notes, settings and the status text. A scroll region that has
  more below the fold fades its last 44px (`.more` mask).

## Elevation & Depth

Flat, tonal and drawn with lines. There are no `box-shadow` drops, blur, glow or glass. Depth comes from three levels:
the ground, a lighter `plate` (operator column, calculator), and `deep` wells for inputs. Boundaries are 1px hairlines
(`line` at rest, `line-hi` when hovered or active). The only box-shadow uses are inset: the 2px signal underline under
the selected account mode, and the inset focus ring on the play plate.

### Named Rules
**The Flat Rule.** Surfaces never lift. A state change is a color, a border color or a 1px nudge (play plate `:active`
moves down 1px), not a shadow.

**The Hairline Rule.** Separation is a 1px `line` or `line-hi` border. Dashed borders mean disabled.

## Shapes

Rectilinear. Every control, input, switch, checkbox, thumb and notch is square (`border-radius: 0`). The single
signature shape is the chamfer: the play plate (and its long variant) is clipped with a 9px cut (`--cut`) on the top-left
and bottom-right corners. Other recurring geometry: a 22px diamond-in-crosshair mark in the brand, 4px diamond
separators (`.sep`) between server data items, a 5px diamond at the end of the leader line, and 1px 5px dash bullets
on release-note lines. Diagrams (terrain, view cone) are line drawings in the same hairline vocabulary.

## Components

All controls are 36px high unless noted, share the hover pattern "border moves from `line` to `line-hi` or `signal`",
and show the same keyboard focus: `outline: 2px solid var(--signal); outline-offset: 2px` (inset variants noted below).

### Play plate (primary button)
- **Shape:** 54px high, full column width, chamfered 9px top-left and bottom-right.
- **Color:** `signal` fill, `signal-ink` text, Condensed 23px caps tracked .2em (text indent compensates tracking).
- **Hover:** `signal-hover` fill. **Active:** `signal-pressed` fill and 1px down. Fill changes in .15s, nudge .12s.
- **Focus:** inset double ring (2px `signal-ink`, then 1px `signal`); no outer outline.
- **Disabled:** `line` fill, `ink-2` text. Used while busy; the only place the play plate is not cyan.
- There is one per form (account form, reset form). Nothing else may use the signal fill at this size.

### Ghost button
- **Style:** transparent, 1px `line-hi` border, `ink` Condensed caps 14px tracked .1em, 14px side padding.
- **Hover:** border to `signal`, 8% cyan wash. **Active:** 18% wash. **Disabled:** `ink-3` text, `line` dashed border.
- Used for secondary actions beside a field: Browse, Calculate, Apply.

### Text link button (`.link`)
- 14px `ink-2` text with a `line-hi` underline offset 3px; hover turns `signal` with a solid underline; disabled `ink-3`,
  no underline. Used for "Forgot your password?" and "Back".

### Inputs and selects
- **Style:** 36px, `deep` well, 1px `line` border, square, `ink` text, `ink-3` placeholders, cyan caret.
- **Hover:** border `line-hi`. **Focus:** border `signal` (no offset ring). **Disabled:** `ink-3` text, dashed border.
- **Select:** native `appearance: none`, 30px right padding, an inline-SVG chevron in `ink-2`; its option list is a `plate`
  surface.
- **Checkbox:** 16px square, `line-hi` border on `deep`; checked is a `signal` fill with a `signal-ink` check mark;
  disabled is dashed at 60% opacity. The row is 36px high with a 9px gap, label in `ink` 15px.
- **Range (FOV):** 10px track with nine tick marks every tenth, 9 x 22px square `signal` thumb; disabled thumb `ink-3`
  and dashed track. Its readout is 17px Condensed `ink`, right-aligned in a fixed 104px.

### Segmented switches
- **Account mode** (Sign in / New account): two equal cells in a 1px `line-hi` frame, 34px high, Condensed caps 14px.
  Selected: `line` fill, `ink` text, 2px inset `signal` underline. Hover: text to `ink`. Radio inputs are visually hidden
  and the focus ring is drawn inside the cell (`outline-offset: -5px`).
- **Language** (ES / EN): same pattern in the top bar, 13px, 1px `line` frame, selected cell filled with `line`.

### Tabs
- Two top-bar tabs, Condensed caps 15px tracked .12em, 16px side padding, `ink-3` at rest, `ink` on hover and when
  selected. The selected tab draws a 2px `signal` underline on the bar's bottom border that scales in from the left
  (.28s). Focus ring is inset (`outline-offset: -6px`).

### Release note
- A row with a 92px meta column (date in `ink-2`, tag in Condensed caps 12px, "New" in `fresh`) and a body column (title
  600 15px, bullets in `ink-2` 14px with a 5px hairline dash). Rows are separated by a 1px `line` top border. The list
  scrolls inside a `deep` well; it is focusable (`tabindex=0`) and text is selectable. Note text is inserted as text,
  never HTML.

### Progress gauge and status
- A 10px `deep` track with a 1px `line-hi` border and a tick every 10%, a `signal` fill that scales on X (.2s linear),
  and a 44px right-aligned percentage in Condensed. With no measurable progress the bar shows a 30%-wide block that
  sweeps (1.3s, infinite). The status text beneath is `ink-2` 14px; `.fault` is `fault`, `.good` is `signal`. The block is
  `aria-live="polite"` and its height is fixed at 78px so it never moves the play plate.

### Server tag (signature)
- The leader label on the terrain: a 90 x 46px leader line ending in a diamond, `signal` stroke, then a name in
  Condensed caps 17px `ink` and a data line (`ink-2`, 14px) with a diamond separator: server address, place and refresh
  rate. It is pointer-transparent and sits over the terrain's top-left.

### Terrain visor (signature)
- 22 rows by 72 columns of a generated height field drawn as SVG polylines, far to near, each row filled with `ground`
  so it hides the rows behind it. Lines are 1px non-scaling `line-hi`; lit lines are `signal`.

### View cone (signature, Settings)
- A 280 x 150 SVG: `line` range rings, dashed `ink-3` stock-FOV outline, a 13% cyan wedge with a 1.5px `signal` edge for
  the chosen FOV, a `signal` leader and a 13 to 16px label, and an `ink` marker for the player. When client mods are off
  it goes grey (`.off`: wedge removed, edge and leader to `ink-3`). Wedge and edge animate their path in .45s.

### Links and rail
- 13.5px `ink-2` buttons with 14px icon where present; hover is `signal`. The support link is `ink` at rest with a heart
  outline icon; it sits with the other links, never between the player and the play plate.

## Motion

Easing is one curve, `--out: cubic-bezier(.16, 1, .3, 1)`. State transitions are .12 to .28s on color, border and
transform only. The one authored moment is the home view arrival: the panel wipes in left to right (.42s), the terrain
lines draw in (stroke-dashoffset, .9s, staggered 28ms per row), then the server tag locks in after .75s (.5s fade with a
left-to-right clip). `prefers-reduced-motion: reduce` turns off all animation and transition, shows the terrain and tag
immediately, and replaces the sweep block with a static 45% bar.

### Named Rules
**The One Moment Rule.** The terrain-and-tag reveal is the only choreographed animation. Everything else is a state
response of .3s or less. New surfaces get no entrance animation of their own.

## Do's and Don'ts

### Do:
- **Do** use `signal` only for live, chosen or focused things, and `fresh` and `fault` only for new and failed.
- **Do** keep every control square, 36px high (34 for the account switch), with a 1px hairline border.
- **Do** keep disabled as `ink-3` text with a dashed border, never as opacity alone.
- **Do** keep the operator column and the play plate on screen on every view; new features go in the field, not in the
  column.
- **Do** keep the status block at a fixed height and give errors in plain words saying what to do next, in the active
  language (strings live in both language tables in `main.js`).
- **Do** keep a visible `signal` focus ring on every interactive element; inset it where an outer ring would clip.
- **Do** keep the page fully local: bundled fonts, inline SVG, no external requests.
- **Do** give any new scroll region a thin `line-hi` scrollbar and, when it has more below, the `.more` fade.

### Don't:
- **Don't** add shadows, blur, glow, glass, gradients as decoration, or rounded corners.
- **Don't** put key art, a hero image, Ubisoft artwork or a game logotype anywhere; the identity is drawn lines.
- **Don't** add a second accent color or a light theme.
- **Don't** put a donation or promotional element between the player and the play plate.
- **Don't** set sentences in uppercase or in the condensed label face; do not set labels in the text face.
- **Don't** add breakpoints or reflow logic; the window is fixed.
- **Don't** add entrance animations to new surfaces.
- **Don't** drop hint or placeholder text below `ink-3` (5.5:1 on `ground`).

## Not canonized (defects or one-offs the build carries)
- The tiny tracked-caps tags above note rows (`note-tag`, `note-new` at 12px, and the 13px "N new" count) read like
  eyebrows. They are data labels in the build; do not copy the pattern as a label-above-heading device on new surfaces.
- The two play-plate state fills (`signal-hover`, `signal-pressed`) are hard-coded hex, not derived from `signal`.
- The 92px note meta column, 340px operator width, 214px visor height and 78px status height are layout facts of this
  window, not reusable spacing tokens.
