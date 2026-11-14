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
easyshot agent capture --window firefox --temp --json
easyshot agent record start --fullscreen --audio none --fps 30 --keystrokes --json
easyshot agent export /tmp/demo.mp4 --look mono --brightness 0.1 --force --json
easyshot agent annotate /tmp/shot.png --ops /tmp/ops.json --out /tmp/marked.png --force --json
```

`agent config get` reads one setting of the user's `[agent]` section: `enabled`, `consent`,
`allowed_outputs`, `allowed_windows`, `redact_secrets`, `keep_captures`, `log_limit`. You may read them. Changing
them is the user's decision.

## Capture, OCR and record flags

- `capture`: `--out PATH` (absolute `.png`, folder exists) also writes a copy; `--delay SECONDS` (0 to 60) waits first, for a menu; `--ui` shows the card; `--temp` skips the gallery.
- `ocr`: one of `--output`, `--geometry`, `--window`, `--fullscreen` or `--file PATH` (PNG or JPEG); `--lang eng+deu`. The answer holds `text`, `lines` (position, `confidence`), `qr` and `source`. Text needs Tesseract; without it and with no QR code the command exits 6 with an install hint.
- `record start`: `--audio system|mic|both|none`, `--fps 30|60`, `--keystrokes`, `--max-seconds N` (1 to 600, default 120). A recording ends by itself after that and when access ends; `stop` still returns its path. `stop` and `discard` answer `not_found` (exit 5) when none of yours runs.

## Export options

```
easyshot agent export /tmp/demo.webm --format gif --size 480p --trim 2..8 --out /tmp/demo.gif --json
easyshot agent export /path/to/video.mp4 --format webm --quality small --json
```

`--format mp4|webm|gif`, `--size original|1080p|720p|480p` (never enlarges), `--quality high|balanced|small`, `--trim A..B` seconds (`..10` and `2.5..` work), `--look natural|vivid|muted|mono`, `--brightness AMOUNT` (-0.3 to 0.3), `--out PATH`, `--force` to replace a file. Needs `ffmpeg` and `ffprobe`. The answer holds `path`, `format`, `width`, `height`, `duration_ms`, `bytes`.

## Annotate

```
easyshot agent annotate /tmp/shot.png --ops /tmp/ops.json --out /tmp/marked.png --json
```

Runs without the daemon or agent access; refuses an existing `--out` unless `--force`. A bad op exits 4 and the message names its index and field.

```json
{"version":1,"ops":[
  {"op":"arrow","from":[120,400],"to":[520,310],"color":"#eb5757"},
  {"op":"counter","x":100,"y":415}
]}
```

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
