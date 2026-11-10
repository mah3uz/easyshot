# es-store

The only writer of every file: atomic writes, config IO, state, history index, decks, presets, retention, and a recording folder's `capture.json`, `input.json` and `project.json`.

May depend on: es-model.

Must not depend on: gpui*, es-ui, any layer-3 crate.
