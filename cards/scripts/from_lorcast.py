#!/usr/bin/env python3
"""Generate per-set engine card TOML from a Lorcast JSON research dump.

Usage:
    python3 cards/scripts/from_lorcast.py /path/to/all_cards.json

The Lorcast JSON itself is an *external research datasource* and is never
committed; this script (and its committed TOML output under ``cards/sets/``) is
what makes the generation reproducible.

Only the *structured* characteristics + supported keywords are emitted. A card's
text-based triggered / activated / static abilities are authored later by hand
via the effect DSL, so no ``[[card.abilities]]`` / ``activated`` / ``statics``
tables are produced here.

The engine TOML loader (``src/domain/cards/loader.rs``) is the source of truth
for field names, validation, and the set of supported keywords.
"""

from __future__ import annotations

import collections
import json
import os
import re
import sys

# Repo paths.
SCRIPT_DIR = os.path.dirname(os.path.abspath(__file__))
REPO_ROOT = os.path.dirname(os.path.dirname(SCRIPT_DIR))
OUT_DIR = os.path.join(REPO_ROOT, "cards", "sets")

# Keywords the loader supports (see `keyword_from` in loader.rs).
VALUELESS = {
    "Evasive",
    "Bodyguard",
    "Rush",
    "Alert",
    "Ward",
    "Reckless",
    "Vanish",
    "Support",
}
# Valued keyword name -> regex capturing the value from a text line. Each
# Lorcana keyword is printed at the start of its own line (e.g. "Challenger +2
# (...)" / "Shift 5 (...)"), so anchoring at the start of a line avoids matching
# classification/alternative-cost variants such as "Puppy Shift 3" or
# "Shift: Discard a card" which the loader does not model.
VALUED = {
    "Challenger": r"^Challenger \+?(\d+)",
    "Resist": r"^Resist \+?(\d+)",
    "Singer": r"^Singer \+?(\d+)",
    "Boost": r"^Boost \+?(\d+)",
    "Shift": r"^Shift \+?(\d+)",
    "Sing Together": r"^Sing Together \+?(\d+)",
}

# Case-insensitive lookup of canonical keyword names (the data has stray
# lowercase entries like "shift" / "bodyguard").
_CANON = {k.lower(): k for k in list(VALUELESS) + list(VALUED)}


def toml_escape(value: str) -> str:
    """Escape a string for a TOML basic string (double-quoted)."""
    out = []
    for ch in value:
        if ch == "\\":
            out.append("\\\\")
        elif ch == '"':
            out.append('\\"')
        elif ch == "\n":
            out.append("\\n")
        elif ch == "\t":
            out.append("\\t")
        elif ch == "\r":
            out.append("\\r")
        elif ord(ch) < 0x20:
            out.append(f"\\u{ord(ch):04X}")
        else:
            out.append(ch)
    return '"' + "".join(out) + '"'


def toml_str(value: str) -> str:
    return toml_escape(value)


def toml_str_array(values) -> str:
    return "[" + ", ".join(toml_escape(v) for v in values) + "]"


def map_type(types):
    """Map the Lorcast ``type`` array to our single type string."""
    t = list(types or [])
    if t == ["Action", "Song"] or ("Action" in t and "Song" in t):
        return "Song"
    if t == ["Action"]:
        return "Action"
    if t == ["Character"]:
        return "Character"
    if t == ["Item"]:
        return "Item"
    if t == ["Location"]:
        return "Location"
    return None


def map_keywords(card):
    """Return the list of supported keyword strings for a card (omitting any
    keyword whose value can't be confidently parsed or that we don't model)."""
    names = card.get("keywords") or []
    text = card.get("text") or ""
    lines = text.split("\n")
    result = []
    for raw in names:
        canon = _CANON.get(raw.strip().lower())
        if canon is None:
            continue
        if canon in VALUELESS:
            result.append(canon)
            continue
        pattern = VALUED[canon]
        value = None
        for line in lines:
            m = re.match(pattern, line)
            if m:
                value = m.group(1)
                break
        if value is None:
            # Could not confidently parse the value -> omit (better than wrong).
            continue
        result.append(f"{canon} {value}")
    return result


def card_to_toml(card, skipped):
    """Render one card as a ``[[card]]`` table, or None if it must be skipped."""
    name = card.get("name")
    version = card.get("version")
    full_name = f"{name} - {version}" if version else name
    kind = map_type(card.get("type"))
    if kind is None:
        skipped.append((full_name, f"unmappable type {card.get('type')!r}"))
        return None
    cost = card.get("cost")
    if cost is None:
        skipped.append((full_name, "missing cost"))
        return None

    strength = card.get("strength")
    willpower = card.get("willpower")
    lore = card.get("lore")
    move_cost = card.get("move_cost")

    # Required-stat validation mirroring the loader.
    if kind == "Character" and (strength is None or willpower is None or lore is None):
        skipped.append((full_name, "Character missing strength/willpower/lore"))
        return None
    if kind == "Location" and (willpower is None or lore is None or move_cost is None):
        skipped.append((full_name, "Location missing willpower/lore/move_cost"))
        return None

    lines = ["[[card]]"]
    lines.append(f"name = {toml_str(full_name)}")
    lines.append(f"type = {toml_str(kind)}")
    lines.append(f"cost = {int(cost)}")
    if card.get("inkwell"):
        lines.append("inkwell = true")
    if kind == "Character":
        lines.append(f"strength = {int(strength)}")
        lines.append(f"willpower = {int(willpower)}")
        lines.append(f"lore = {int(lore)}")
    elif kind == "Location":
        lines.append(f"move_cost = {int(move_cost)}")
        lines.append(f"willpower = {int(willpower)}")
        lines.append(f"lore = {int(lore)}")
    classifications = card.get("classifications") or []
    if classifications:
        lines.append(f"classifications = {toml_str_array(classifications)}")
    keywords = map_keywords(card)
    if keywords:
        lines.append(f"keywords = {toml_str_array(keywords)}")
    # The printed rules text, as a comment, to author the effect DSL from later.
    text = (card.get("text") or "").strip()
    if text:
        lines.append("# text:")
        for text_line in text.split("\n"):
            lines.append(f"#   {text_line}")
    return "\n".join(lines)


def main(argv):
    if len(argv) != 2:
        print(f"usage: {argv[0]} <lorcast_json_path>", file=sys.stderr)
        return 2
    with open(argv[1], "r", encoding="utf-8") as fh:
        cards = json.load(fh)

    by_set = collections.OrderedDict()
    for card in cards:
        code = card.get("set", {}).get("code")
        if code is None:
            continue
        by_set.setdefault(code, []).append(card)

    os.makedirs(OUT_DIR, exist_ok=True)
    skipped = []
    total_written = 0
    files = 0
    for code, group in by_set.items():
        blocks = []
        for card in group:
            block = card_to_toml(card, skipped)
            if block is not None:
                blocks.append(block)
        if not blocks:
            continue
        path = os.path.join(OUT_DIR, f"{code.lower()}.toml")
        header = (
            f"# Cards for Lorcana set {code}, in the engine's own TOML format.\n"
            f"# Generated by cards/scripts/from_lorcast.py from a Lorcast research\n"
            f"# dump (the external dataset itself is not committed). Structured\n"
            f"# fields + supported keywords only; text-based abilities are authored\n"
            f"# by hand via the effect DSL.\n\n"
        )
        with open(path, "w", encoding="utf-8") as fh:
            fh.write(header)
            fh.write("\n\n".join(blocks))
            fh.write("\n")
        files += 1
        total_written += len(blocks)

    print(f"wrote {files} files, {total_written} cards")
    print(f"skipped {len(skipped)} cards")
    for name, reason in skipped:
        print(f"  - {name}: {reason}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
