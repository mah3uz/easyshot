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
easyshot agent capture --fullscreen --no-ui --out /tmp/screen.png --json
easyshot agent record start --fullscreen --audio none --fps 30 --keystrokes --json
easyshot agent export /tmp/demo.mp4 --look mono --brightness 0.1 --force --json
easyshot agent annotate /tmp/shot.png --ops /tmp/ops.json --out /tmp/marked.png --force --json
```

`agent config get` reads one setting of the user's `[agent]` section: `enabled`, `consent`,
`allowed_outputs`, `allowed_windows`, `redact_secrets`, `log_limit`. You may read them. Changing
them is the user's decision.

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
```

`kind` is `outputs`, `windows`, `capture`, `export`, `annotate`, `ocr`, `recording`, `access`, `config`, or
`usage` when the command line itself could not be read.

## Limits

- `agent capture` takes one still frame; nothing watches the screen between calls.
- `agent record` is the one command that keeps running after it answers.
- The user's scope (`allowed_outputs`, `allowed_windows`) can hide outputs and windows from you
  and refuses commands that reach others with exit 8.
- Only the user who runs the daemon is answered.
