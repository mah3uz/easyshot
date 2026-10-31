# Bundled fonts

All fonts are licensed under the SIL Open Font License 1.1; texts in `LICENSES/`.

| File | Upstream | SHA-256 |
|---|---|---|
| Inter-Regular.ttf, Inter-Bold.ttf, Inter-Italic.ttf, Inter-BoldItalic.ttf | Inter 4.1 static TTFs, `extras/ttf/` in https://github.com/rsms/inter/releases/download/v4.1/Inter-4.1.zip | see below |
| InterTight-{Regular,Bold,Italic,BoldItalic,SemiBold}.ttf | Inter Tight 3.x static TTFs, `fonts/ttf/` of https://github.com/rsms/inter-tight at commit c194f94c60b569b47876811321f5ef1f0c2614a2 | see below |
| Nunito[wght].ttf, Nunito-Italic[wght].ttf | VARIABLE fonts (wght 200-1000); `fonts/variable/` of https://github.com/googlefonts/nunito at commit 8c6a9bb9732545b9ed53f29ec5e1ab0ff53c4e6f. Upstream publishes no static Nunito (`buildStatic: false`). | see below |

Nunito Regular, Bold, Italic and Bold Italic are the roman and italic files at wght 400 and 700.

```
40d692fce188e4471e2b3cba937be967878f631ad3ebbbdcd587687c7ebe0c82  Inter-Regular.ttf
288316099b1e0a47a4716d159098005eef7c0066921f34e3200393dbdb01947f  Inter-Bold.ttf
bbc051dd204b5019a1aa0bc0ae2aa8a05ab13e7a3f979fa357631dc7feb6833a  Inter-Italic.ttf
948405a16cdc62701da5f4005ed068ca5f4d27061d98f7974ccfc37831d9581d  Inter-BoldItalic.ttf
d85c374020716713e4ff2ff223d602919a1bb38f940205ea75b404859f0313a3  InterTight-Regular.ttf
54b8da2868f28324213e89607255c90e1ff8f2ff156b658ed8cf347f39b23843  InterTight-Bold.ttf
2c4d87bfe92696a781abb56a69f910d376bd5282eb8d3b0c2debbca210f1cfbe  InterTight-Italic.ttf
2bb58d0fc6a0601e5ce5980677443d900e82cedad885070ad8e800b8ad1ba89b  InterTight-BoldItalic.ttf
975a7f22d6e9a45a7989e6d3cbec6ff5b3089cc2272099ac95638b36d4fc948a  InterTight-SemiBold.ttf
bb55a5ca5c2042335b3991af27c4d0705d0ef41cac6164ac737fd8f2a1e85207  Nunito[wght].ttf
b520cc871868b0acfca1beda875df7f4a44ebce914f8a89f83977fc9c09529c8  Nunito-Italic[wght].ttf
```

OFL texts: `LICENSES/Inter-OFL-1.1.txt` (from the Inter 4.1 zip), `LICENSES/InterTight-OFL-1.1.txt` and `LICENSES/Nunito-OFL-1.1.txt` (from the upstream repos at the commits above).
