#!/usr/bin/env python3
"""Fail on a Fluent id or an icon that nothing references.

An id counts as used when its string appears as a literal in any crate source, when it is the
`-message` companion of a used toast id, when it is `shortcut-<id>` for a catalogue id, or when
another message points at it. Entries in scripts/unused-ids.allow (`id-or-prefix*  reason`) are kept
on purpose. An icon counts as used when its variant is named as `IconName::<Variant>`, or when a
toast names a gpui_component icon of the same file stem.
"""
import glob
import os
import re
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
os.chdir(ROOT)

sources = {f: open(f).read() for f in glob.glob("crates/**/*.rs", recursive=True)}
literals = set()
for text in sources.values():
    literals.update(m.group(1) for m in re.finditer(r'"((?:[^"\\]|\\.)*)"', text))

allow = []
for line in open("scripts/unused-ids.allow"):
    line = line.strip()
    if line and not line.startswith("#"):
        allow.append(line.split(None, 1)[0])


def allowed(name):
    return any(name.startswith(a[:-1]) if a.endswith("*") else name == a for a in allow)


problems = []
used_allow = set()

for ftl in sorted(glob.glob("i18n/en-US/*.ftl")):
    text = open(ftl).read()
    for ident in re.findall(r"^(-?[A-Za-z][\w-]*)\s*=", text, re.M):
        if ident in literals or ident.removesuffix("-message") in literals:
            continue
        if ident.startswith("shortcut-") and ident[len("shortcut-"):] in literals:
            continue
        if re.search(r"\{\s*" + re.escape(ident) + r"\b", text):
            continue
        if allowed(ident):
            continue
        problems.append(f"{ftl}: id \"{ident}\" is defined but never referenced")

icon_src = sources["crates/es-ui/src/icons.rs"]
body = icon_src.split("icons! {", 1)[1].split("\n}\n", 1)[0]
icons = re.findall(r"^\s*(\w+) = \"([^\"]+)\",", body, re.M)
named = set()
for path, text in sources.items():
    if path != "crates/es-ui/src/icons.rs":
        named.update(re.findall(r"\bIconName::(\w+)", text))
kebabs = {re.sub(r"(?<!^)(?=[A-Z])", "-", n).lower() for n in named}
for variant, name in icons:
    if variant in named or name in kebabs or allowed("icon:" + variant):
        continue
    problems.append(f"icon IconName::{variant} (\"{name}\") is never used")

for p in problems:
    print("unused-ids:", p, file=sys.stderr)
if problems:
    sys.exit(1)
print("unused-ids: ok")
