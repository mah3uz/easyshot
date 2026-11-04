# easyshot — design

Why the interface looks and behaves as it does. How it is built is in `ARCHITECTURE.md`;
each choice and what was rejected is in `DECISIONS.md`.

## 1. Reference design

The toolbar, the inspector and every surface follow a fixed reference design and motion
table. Where behaviour is a deliberate choice it is listed in `DECISIONS.md`, and anything
else that looks different from the reference is a bug. The v1 feature list is in `MANUAL.md`.

## 2. Add behaviour, never chrome

Additions to the reference design add behaviour only. No new buttons, panels or fields. Each
addition has its own catalogue strings, and each keyboard addition is listed in Settings >
Shortcuts. "Off" switches are config keys only, not interface.

| Addition | Behaviour |
|---|---|
| Arrow-key nudge | 1 page pixel; Shift = 10 (`[editor] nudge_step`, `nudge_step_shift`). Key repeat allowed for arrows only. One undo entry per key-hold. |
| Esc deselects | In Select with nothing being edited or cropped and no dialog open, Esc clears the selection. A second Esc does nothing; it never closes the window. |
| Middle-button drag and Space-hold pan | Open and closed hand cursors. Space is inert while editing text and ignores key repeat. With Space held, dragging an annotation moves it, with any tool. |
| Pointer-anchored zoom | Ctrl/Alt+wheel and pinch zoom about the pointer. Keyboard and menu zoom stay centre-anchored. `[editor] zoom_anchor="centre"` restores centre-anchored zoom. |
| Duplicate | Ctrl+D duplicates the selection at a +16 page pixel offset. Bindings between duplicated shapes are remapped. One undo entry. |
| Copy and paste shapes | Ctrl+C copies shapes only when the selection is non-empty and Select is active, using an in-app clipboard, never the system clipboard (which holds images). Ctrl+V pastes at +16, or at the pointer if it is over the canvas. Image copy stays Ctrl+Shift+C. |
| Z-order keys | Ctrl+] / Ctrl+[ one step; Ctrl+Shift+] / [ to front or back. Reordering applies only among non-redaction shapes; the draw order redaction, spotlight, shapes is fixed. |
| Tab and Shift+Tab | Cycle the selection through shapes in z-order when the canvas has focus, which helps keyboard and screen-reader users. |
| Shell robustness | An explicit capture queue, hand-edit-safe config, doctor and provider fallbacks. |

Not added, because they would break the 1:1 look or need tuning: snapping guides (maybe
later, behind a setting), an eraser tool (a 13th button), Alt-drag duplicate (Alt already
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
  Highlight, Pixelate and Blur.
- Text shows the system I-beam; shapes, lines and arrows show annotationPlus.
- windowCamera: the Lucide `camera` at 18 px, black with a 1.5 px white halo, hotspot at
  the centre.

## 5. Motion

A change takes effect the moment you make it; only its drawing moves, so a click or a key
pressed during a motion acts on the new state. Every motion is timed from one table of
tokens (`es-ui` `motion`), so the same kind of change always moves the same way. "Snappy"
is a fast start that settles softly, `cubic-bezier(.2, .8, .2, 1)`.

| Motion | Token | Time and easing |
|---|---|---|
| Editor: after Crop, the kept region grows from the crop rectangle to fill the view | `CROP_COMMIT` | 220 ms, snappy |
| Editor: a preset, or a canvas shape, draws its padding, corners, shadow and shape into place | `PRESET_APPLY` | 200 ms, snappy |
| Editor: picking an alignment slides the card to its new place | `SETTLE` | 180 ms, snappy |
| Editor: switching the fill library, or the wallpaper source, fades the new swatches in | `FILL_SWITCH` | 160 ms, snappy |
| Deck: a card's picture fades in over "Loading preview…", and "Saving…" fades in and out | `CROSSFADE` | 150 ms, ease-out |
| Editor: when a camera or blur change settles, the export-exact picture fades in over the one it replaces | `COMPOSITE_FADE` | 180 ms, ease-out |
| Settings home: tiles and recent rows fill in under the pointer and fade back after it | `HOVER_FILL` | 120 ms fade, ease-out |
| Settings, and the editor's inspector switching to or from a tool's options: a page rises 8 px into place and fades in | `PAGE_CHANGE` | 180 ms, ease-out |
| Introduction: a step slides in from the side you are heading to | `STEP_CHANGE` | 200 ms, ease-out |
| Tooltips open after a rest, and at once while you move straight on | `TOOLTIP_DELAY`, `TOOLTIP_WARM` | 160 ms, 500 ms |

A switch of swatches fades only the new ones in: the old grid leaves at once, so two grids of
different heights never overlap.

The preset and shape motion goes through the one canvas renderer, a step at a time; a look
with the camera or the scene blur is drawn as one finished image, so it changes at once.
Dragging a slider shows each value at once, since your hand is already the motion. While a
camera or blur slider drags, each value is drawn off the UI thread at the picture's full
resolution, the way the export draws it, and the last finished picture stays up until the
next lands, so the preview is never softer than the result.

With Reduce Motion on (from the desktop's setting), anything that moves, and any longer fade,
appears at its end at once. A fade of 120 ms or less carries no movement and stays, so hover
fills still fade.

## 6. Where the rest is recorded

The full per-surface, per-tool cursor table, the editor's visual specifications and the
design tokens are not written down yet.
