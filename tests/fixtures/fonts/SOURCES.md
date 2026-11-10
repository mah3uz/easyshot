# Test fonts

`FallbackBengali.ttf`: Noto Sans Bengali Regular (SIL Open Font License 1.1, from the Arch
`noto-fonts` 2026.11.01 package) cut to the glyphs of "বাংলা" with
`pyftsubset --text="বাংলা" --layout-features='*' --no-hinting --desubroutinize`, then
renamed to the family "Fallback Bengali" with fontTools so no fallback list names it.
Used only by tests, never shipped.
