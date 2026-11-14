# easyshot agent: reference

Run `easyshot agent <command> --help` for the flags of the installed version. `--json` goes
before or after the subcommand.

## Commands

```
easyshot agent status --json
easyshot agent config get redact_secrets --json
easyshot agent list outputs --json
easyshot agent list windows --json
easyshot agent capture --output DP-1 --no-wait --json
easyshot agent capture --fullscreen --out /tmp/screen.png --json
easyshot agent capture --window firefox --temp --json   # not kept in the gallery
easyshot agent record start --fullscreen --audio none --fps 30 --keystrokes --json
easyshot agent export /tmp/demo.mp4 --look mono --brightness 0.1 --force --json
easyshot agent annotate /tmp/shot.png --ops /tmp/ops.json --out /tmp/marked.png --force --json
```

`agent config get` reads one setting of the user's `[agent]` section: `enabled`, `consent`,
`allowed_outputs`, `allowed_windows`, `redact_secrets`, `keep_captures`, `log_limit`. You may read them. Changing
them is the user's decision.

## Annotate ops

The ops file is `{"version":1,"ops":[...],"background":{...}}`; `background` is optional and
`ops` may be empty. Coordinates and sizes are pixels of the input image from its top left
corner; colours are `#rrggbb` or `#rrggbbaa`. Fields in brackets are optional.

| `op` | Fields |
|---|---|
| `arrow` | `from` `[x,y]`, `to` `[x,y]` (at least 1 px apart), [`color`], [`width` 0.5 to the longest edge], [`head`: `none`, `arrow`, `triangle`, `square`, `dot`, `diamond`, `inverted`, `bar`] |
| `rect`, `ellipse` | `x`, `y`, `w`, `h`, [`color`], [`width`], [`fill` true/false] |
| `text` | `x`, `y`, `text` (1 to 2000 characters), [`size` 4 to 2000], [`color`], [`background` colour behind the text], [`font`: `pro`, `compact`, `rounded` or an installed family's name; one not installed draws in `pro`] |
| `counter` | `x`, `y`, [`n` 1 to 9999; numbers count up from 1 when left out], [`color`] |
| `highlight` | `x`, `y`, `w`, `h`, [`color`] |
| `blur`, `pixelate` | `x`, `y`, `w`, `h`, [`amount` 0 to 1] |

`background` puts the image on a card:

| Field | Meaning |
|---|---|
| `padding` | pixels around the image, 0 to twice its longest edge |
| `corners` | corner radius in pixels, 0 to half its shorter edge |
| `color` | one colour, or |
| `gradient` | 2 to 8 colours, top left to bottom right (give `color` or `gradient`, not both; neither gives graphite) |
| `shadow` | 0 to 1 |

```json
{"version":1,"ops":[],"background":{"gradient":["#0f3b46","#1fa3a0","#5ee6c8"],"padding":72,"corners":16,"shadow":0.5}}
```

The output is larger than the input by the padding on each side; read `width` and `height` from
the answer.

## JSON shapes

Every answer is one object with `ok` and `kind`. Fields may be added; existing ones keep their
meaning.

```
{"ok":true,"kind":"outputs","outputs":[{"name":"DP-1","description":"Dell U2720Q","x":0,"y":0,"width":2560,"height":1440,"scale":1.5,"focused":true}]}
{"ok":true,"kind":"capture","path":"/home/u/.local/share/easyshot/library/shot.png","width":480,"height":300,"scale":1.5,"output":"DP-1"}
{"ok":true,"kind":"recording","id":"...","state":"recording","target":"...","max_seconds":120}
{"ok":true,"kind":"annotate","path":"/tmp/marked.png","width":1280,"height":720,"ops":2}
{"ok":true,"kind":"export","path":"/tmp/demo.gif","format":"gif","width":480,"height":270,"duration_ms":6000,"bytes":812345}
{"ok":false,"kind":"windows","error":{"code":"unsupported","message":"this desktop gives easyshot no window list"}}
{"ok":false,"kind":"capture","error":{"code":"not_found","message":"no window matches \"nosuchapp\"","hint":"No open window is called \"nosuchapp\". Open windows: Terminal (foot), Mozilla Firefox (firefox)."}}
```

`error.hint` is optional: one plain sentence for the user, set where the cause is known. Tell
the user that sentence as it is. `error.message` is the technical text for you.

`kind` is `outputs`, `windows`, `capture`, `export`, `annotate`, `ocr`, `recording`, `access`, `config`, or
`usage` when the command line itself could not be read.

## Limits

- `agent capture` takes one still frame; nothing watches the screen between calls.
- `agent record` is the one command that keeps running after it answers.
- The user's scope (`allowed_outputs`, `allowed_windows`) can hide outputs and windows from you
  and refuses commands that reach others with exit 8.
- Only the user who runs the daemon is answered.
