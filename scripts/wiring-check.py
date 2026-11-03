#!/usr/bin/env python3
"""Fail on registration code that no real entry point reaches.

Two mechanical cases, both read from non-test code only (a file is cut at its inline `#[cfg(test)] mod x {`;
tests/, examples/, benches/ and *tests.rs files are skipped):

  * a `pub fn` named init*, install*, register*, bind*, attach* or follow* that nothing calls;
  * an `impl EventEmitter<T>` whose event type T is never received (`&T` in a file that subscribes).

Intended exceptions go in scripts/wiring-check.allow, one per line: `fn:<name>` or `event:<Type>`,
then why.
"""
import glob
import os
import re
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
os.chdir(ROOT)

SKIP = re.compile(r"(^|/)(tests|examples|benches|vendor)/|(^|/)[\w-]*tests\.rs$")
sources = {}
for path in glob.glob("crates/**/*.rs", recursive=True):
    if SKIP.search(path):
        continue
    text = open(path).read()
    inline_tests = re.search(r"#\[cfg\(test\)\]\s*\n\s*mod\s+\w+\s*\{", text)
    sources[path] = text[: inline_tests.start()] if inline_tests else text

allow = set()
for line in open("scripts/wiring-check.allow"):
    line = line.strip()
    if line and not line.startswith("#"):
        allow.add(line.split(None, 1)[0])

problems = []
used_allow = set()


def check(key, message):
    if key in allow:
        used_allow.add(key)
    else:
        problems.append(f"{message}  (allow with `{key}  reason` in scripts/wiring-check.allow)")


DEF = re.compile(r"^\s*pub(?:\([a-z]+\))?\s+(?:const\s+)?fn\s+((?:init|install|register|bind|attach|follow)\w*)", re.M)
everything = "\n".join(sources.values())
for path, text in sorted(sources.items()):
    for m in DEF.finditer(text):
        name = m.group(1)
        calls = len(re.findall(r"\b" + name + r"\s*(?:::<[^>]*>)?\s*\(", everything))
        refs = len(re.findall(r"\b" + name + r"\b", everything))
        defs = len(re.findall(r"\bfn\s+" + name + r"\b", everything))
        if calls - defs <= 0 and refs - defs <= 0:
            check(f"fn:{name}", f"{path}: `{name}` is never called")

IMPL = re.compile(r"impl\s+EventEmitter<\s*([A-Za-z_][\w:]*)\s*>\s+for\s+\w+")
for path, text in sorted(sources.items()):
    for m in IMPL.finditer(text):
        event = m.group(1).split("::")[-1]
        received = any(
            re.search(r"&\s*" + event + r"\b", t) and re.search(r"\b(subscribe|subscribe_in|observe)\w*\s*\(", t)
            for t in sources.values()
        )
        if not received:
            check(f"event:{event}", f"{path}: nothing subscribes to `{event}`")

for key in sorted(allow - used_allow):
    problems.append(f"scripts/wiring-check.allow: `{key}` no longer needs an exception; remove it")

if problems:
    print("wiring-check failed:")
    for p in problems:
        print("  " + p)
    sys.exit(1)
print("wiring-check ok")
