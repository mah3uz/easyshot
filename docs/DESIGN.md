# easyshot — design

Why the interface looks and behaves as it does. How it is built is in `ARCHITECTURE.md`;
each choice and what was rejected is in `DECISIONS.md`.

## 1. Reference design

The toolbar, the inspector and every surface follow a fixed reference design and motion
table. Where behaviour is a deliberate choice it is listed in `DECISIONS.md`, and anything
else that looks different from the reference is a bug. The feature list is `FEATURES.md`; the details are in `MANUAL.md`.

## 2. Add behaviour, never chrome

Additions to the reference design add behaviour only. No new buttons, panels or fields. The one exception is the share popover, which sharing needs (`DECISIONS.md`, Sharing). Each
addition has its own catalogue strings. Its keys are fixed and listed in the manual's Editor
keys table, not in Settings > Shortcuts. "Off" switches are config keys only, not interface.

| Addition | Behaviour |
|---|---|
| Arrow-key nudge | 1 page pixel; Shift = 10 (`[editor] nudge_step`, `nudge_step_shift`). Key repeat allowed for arrows only. One undo entry per key-hold. |
| Esc steps back | One level per press: finish the text being typed, leave crop, clear the selection, then switch to the Select tool. It never closes the window. |
| Middle-button drag and Space-hold pan | Open and closed hand cursors. Space is inert while editing text and ignores key repeat. With Space held, dragging an annotation moves it, with any tool. |
| Pointer-anchored zoom | Ctrl/Alt+wheel and pinch zoom about the pointer. Keyboard and menu zoom stay centre-anchored. `[editor] zoom_anchor="centre"` restores centre-anchored zoom. |
| Duplicate | Ctrl+D duplicates the selection at a +16 page pixel offset. Bindings between duplicated shapes are remapped. One undo entry. |
| Copy and paste shapes | Ctrl+C copies shapes only when the selection is non-empty and Select is active, using an in-app clipboard, never the system clipboard (which holds images). Ctrl+V pastes at +16, or at the pointer if it is over the canvas. Image copy stays Ctrl+Shift+C. |
| Z-order keys | Ctrl+] / Ctrl+[ one step; Ctrl+Shift+] / [ to front or back. Reordering applies only among non-redaction shapes; the draw order redaction, spotlight, shapes is fixed. |
| Tab and Shift+Tab | Cycle the selection through shapes in z-order when the canvas has focus, which helps keyboard and screen-reader users. |
| Shell robustness | One capture at a time (a new pick replaces the open one, a committed one refuses), hand-edit-safe config, doctor and provider fallbacks. |

Not added, because they would break the 1:1 look or need tuning: snapping guides, an eraser tool (a 14th button), Alt-drag duplicate (Alt already
means centre-resize), region selection across outputs, and numeric geometry fields.

## 3. Tool options

A tool's options are a page of the inspector: a back arrow and the tool's name where the
preset bar sits, then labelled sections in the inspector's own label style, spacing and
fills, so nothing covers the image you are drawing on. Picking a tool brings its page
forward and opens a hidden inspector; a new selection shows its page without opening one.
The page stays until the tool changes or you go back, so you can draw, restyle and draw
again. Clicking the armed tool keeps it and brings its page back. The page rises in with
`PAGE_CHANGE`, as Settings pages do. The same page holds crop's ratio, Reset, Cancel and
Apply, so the title bar keeps Save and Export only.

## 4. Cursors

Custom bitmaps are drawn procedurally at 1x, 1.5x and 2x and registered once through
patches to vendored GPUI that add `CursorStyle::Custom`; each custom cursor has its own
Wayland surface, and its image is committed before the cursor is set. The pointer is never
hidden on key presses. The remaining cursors are GPUI
`CursorStyle` values. The reference design's rules are the base.

Affordance cursors are an addition: where the reference design shows no special cursor, easyshot
shows resize, rotate, endpoint, bend, crop and pin cursors, and an arrow during marquee.
This was a decision of the maintainer (2026-10-31) and is a deviation (`DECISIONS.md`).

The resize cursor follows the handle's direction from the selection centre in screen space
(including the shape's rotation, mirroring and the viewport), snapped to the nearest 45
degrees. The cursor chosen at press is kept for the whole drag so it does not flip
mid-gesture.

Custom bitmaps:

- annotationPlus: 22x22 logical, hotspot (11, 11), plus arms of 6, white 5 pt then black
  2 pt.
- rotate: 24x24, hotspot (12, 12), a 90 degree arc of radius 7 with an arrowhead at each
  end, black 2 pt over a white 4.5 pt halo, pre-rendered at 8 orientations. The
  orientation follows the corner's screen-space direction, and while rotating it follows
  the pointer's angle around the selection centre.
- pen: 22x22, hotspot (4, 18) at the lead, a pencil at 45 degrees, for Freehand and the
  Highlighter.
- badge: 22x22, hotspot (11, 11), a black disc of radius 7.5 with a white 1 cut out, for the
  counter.
- area: 22x22, hotspot (4, 4) at a solid corner, the rest of a 14 px square dotted, for
  Highlight.
- eraser: an eraser whose rubber edge holds the hotspot, for Pixelate and Blur.
- Text shows the system I-beam; shapes, lines and arrows show annotationPlus.
- windowCamera: the Lucide `camera` at 18 px, black with a 1.5 px white halo, hotspot at
  the centre.

## 5. Motion

A change takes effect the moment you make it; only its drawing moves, so a click or a key
pressed during a motion acts on the new state. Every motion is timed from one table of
tokens (`es-ui` `motion`), so the same kind of change always moves the same way. "Snappy"
is a fast start that settles softly, `cubic-bezier(.2, .8, .2, 1)`. Something arriving, or a
layout springing open, moves on a damped spring instead: it leaves at rest, passes its end
once and swings back, and the motion is cut when it has come to rest within half a percent.
There are four: the snappy spring (0.3 s swing, 75 % damping, about 3 % past its end, 360
ms), the bouncy spring (0.4 s, 62 %, about 8 %, 600 ms), the gentle spring (0.35 s, 85 %,
under 1 %, 380 ms) and the calm spring, the gentle one half again as long (0.525 s, 85 %, under
1 %, 570 ms); `SNAPPY_SPRING`, `BOUNCY_SPRING`, `GENTLE_SPRING` and `CALM_SPRING`. A whole page
crossing a window moves on the calm spring and a Settings page change takes half again as long
as other page changes: at the faster pace the eye lost the page it was following.

| Motion | Token | Time and easing |
|---|---|---|
| Editor: after Crop, the kept region grows from the crop rectangle to fill the view | `CROP_COMMIT` | 220 ms, snappy |
| Editor: a preset, or a canvas shape, draws its padding, corners, shadow and shape into place | `PRESET_APPLY` | 200 ms, snappy |
| Editor: the first background fill springs its padding and shadow in, swinging just past and back, and taking the fill away springs them out | `STAGE_IN` | 600 ms, bouncy spring |
| Editor: crop Reset, or an aspect change, eases the crop frame from where it was to its new place | `CROP_RESHAPE` | 180 ms, snappy |
| Editor: cancelling a crop widens the frame to the whole image as the frame and the dimming fade | `CROP_COMMIT` | 220 ms, snappy |
| Editor: the Progressive Blur Direction row opens and closes with the Directional mode, dropping 8 px into place as it fades | `SECTION_EXPAND` | 180 ms, snappy |
| Editor: the low-resolution notice slides 8 px in from the left as it fades in, and back out as it fades | `NOTICE_EXPAND` | 220 ms, snappy |
| Editor: a preset's delete button grows from 82 % as it fades in, and shrinks away as it fades | `BUTTON_POP`, `BUTTON_POP_SCALE` | 180 ms, snappy |
| Editor: the focus pad's knob grows from 10 to 12 px as a drag starts, and a double-click glides it back to the centre | `KNOB_GROW`, `SETTLE` | 120 ms, 180 ms, snappy |
| Editor: picking an alignment slides the card to its new place | `SETTLE` | 180 ms, snappy |
| Editor: switching the fill library, or the wallpaper source, fades the new swatches in | `FILL_SWITCH` | 160 ms, snappy |
| Deck: a card's picture fades in over "Loading preview…", and "Saving…" fades in and out | `CROSSFADE` | 150 ms, ease-out |
| Editor: when a camera or blur change settles, the export-exact picture fades in over the one it replaces | `COMPOSITE_FADE` | 180 ms, ease-out |
| Settings home: tiles and recent rows fill in under the pointer and fade back after it | `HOVER_FILL` | 120 ms fade, ease-out |
| Deck: a card's tools fade in under the pointer and fade back out after it | `DECK_HOVER` | 120 ms fade, ease-out |
| Deck: the stack fans out into a column under the pointer and folds back after it; the count and the dimming of the cards behind fade as it goes | `DECK_FAN` | 380 ms, gentle spring |
| Deck: a new card grows from 88 % and rises 14 px into place as it fades in, swinging just past and back; with Reduce Motion it only fades in, in place (`DECK_ENTER_REDUCED`, 120 ms, linear) | `DECK_ENTER`, `DECK_ENTER_SCALE`, `DECK_ENTER_RISE` | 600 ms, bouncy spring |
| Toast: grows from 92 % and rises 6 px into place as it fades in, swinging just past and back; it leaves by shrinking to 96 % as it fades, with no swing; with Reduce Motion it only fades in and out (`TOAST_REDUCED`, 120 ms, linear) | `TOAST_IN`, `TOAST_IN_SCALE`, `TOAST_RISE`, `TOAST_OUT`, `TOAST_OUT_SCALE` | 360 ms snappy spring in, 160 ms ease-in out |
| Recording: the capture bar shrinks to 94 % and fades out as the control bar grows from 94 % and fades in, swinging just past full size; with Reduce Motion the control bar appears at once | `HANDOFF`, `HANDOFF_IN`, `HANDOFF_SCALE` | 200 ms snappy out, 360 ms snappy spring in |
| Recording: the control bar changing in place: a new mute icon, the Stop button's word ("Saving…"), and the Start over or Discard question taking the bar's place (and the bar coming back on Cancel) fade in once the old state has gone; a confirmed question stays, inert, until the bar leaves; with Reduce Motion each shows at once | `BAR_SWAP` | 220 ms fade, ease-out |
| Pin: the close button fades in under the pointer and back out after it | `CLOSE_FADE` (pin) | 150 ms fade, ease-out |
| Settings: the old page fades out while the new one rises 8 px into place and fades in | `SETTINGS_PAGE` | 270 ms, ease-out |
| Editor: the inspector switching to or from a tool's options rises 8 px into place and fades in | `PAGE_CHANGE` | 180 ms, ease-out |
| Introduction: the next step slides in from the side you are heading to as the last one slides out the other way, both fading | `STEP_CHANGE` | 200 ms, ease-out |
| Home window: the new tab slides in from its side (Media Gallery left, Settings right) as the old one slides out the other way, both fading, settling softly | `TAB_SLIDE` | 570 ms, calm spring (the gentle spring, half again as long) |
| The tab switcher's selection glides to the new tab, swinging just past and back | `THUMB_GLIDE` | 360 ms, snappy spring |
| Share: while the file is prepared, the spinner on Share turns, once each 800 ms; with Reduce Motion it stands still | `SPIN` | 800 ms a turn, linear, repeating |
| Countdown: the dial, or the recording badge, grows from 82 % and fades in, swinging just past full size and back | `COUNTDOWN_IN` | 600 ms, bouncy spring |
| Countdown: each new second drops in from above as the last one falls away, both fading | `DIGIT_ROLL` | 300 ms, snappy |
| Countdown: after the last second the dial shrinks to 82 % and fades out | `COUNTDOWN_OUT` | 300 ms, snappy |
| Editor: Saved and Copied fade in, hold, and fade out at the end of the hold; the button they replaced fades back | `FLASH`, `FLASH_HOLD` | 200 ms snappy fades, 1.4 s hold |
| Tooltips open after a rest, on every control, including the next one you move straight on to | `SHOW_DELAY` (`components/tooltip.rs`) | 1.5 s |
| A tooltip hides by itself once it has had time to be read, and stays hidden until the pointer leaves its control | `read_time` (`components/tooltip.rs`) | 2 s + 60 ms a character, at most 8 s |

Tooltips open where the pointer is, not anchored to their control, so one tooltip cannot glide
from control to control; each opens on its own, after the full rest. A tooltip that opened at once
on the next control read as one tooltip jumping across as the pointer passed over a row.
Every tooltip comes from one component, `Tip::tip` in `es-ui`, which holds its look and both
times; `just grep-guard` fails on a tooltip made anywhere else. A tooltip that stayed up as
long as the pointer rested covered the controls beside it; one that comes back without the
pointer leaving would cover them again. Reduce Motion changes no tooltip time.

Elements have no blur filter, so the countdown's entry and a new deck card scale and fade
without a blur. Elements have no scale either: a toast grows its card around its words, which
stay their size and fade in.

A switch of swatches fades only the new ones in: the old grid leaves at once, so two grids of
different heights never overlap. The recording control bar changes the same way: its surface
takes the new size at once, so the old state is gone before the new one fades in. A confirmed
Start over or Discard question stays up until the bar leaves, rather than the bar flashing back
for the moment the recorder takes to stop.

The preset and shape motion goes through the one canvas renderer, a step at a time; a look
with the camera or the scene blur is drawn as one finished image, so it changes at once.
Dragging a slider shows each value at once, since your hand is already the motion. While a
camera or blur slider drags, each value is drawn off the UI thread at the picture's full
resolution, the way the export draws it, and the last finished picture stays up until the
next lands, so the preview is never softer than the result.

With Reduce Motion on (from the desktop's setting), anything that moves, and any longer fade,
appears at its end at once. A fade of 120 ms or less carries no movement and stays, so hover
fills still fade. The countdown keeps a 120 ms fade in and out (`COUNTDOWN_IN_REDUCED`,
`COUNTDOWN_OUT_REDUCED`) with no scale and no digit roll.

## 6. Where the rest is recorded

The full per-surface, per-tool cursor table, the editor's visual specifications and the
design tokens are not written down; the tokens are the `es-ui` source.
