#!/usr/bin/env python3
"""Generate the per-card engine TOML from a Lorcast JSON research dump.

Usage:
    python3 cards/scripts/from_lorcast.py /path/to/all_cards.json

Writes one file per card to ``cards/<set>/<collector_number>.toml`` (the set
directory is the lowercased Lorcast set code). The Lorcast JSON itself is an
*external research datasource* and is never committed; this script + its
committed output is what makes generation reproducible.

Only the *structured* characteristics + supported keywords + printed ``text`` are
emitted. A card's text-based triggered / activated / static abilities are authored
later (an AI pass) via the effect DSL, so no ``[[abilities]]`` / ``activated`` /
``statics`` tables are produced here. Emission is shared with
``combine_sets.py`` via ``card_io``.
"""

from __future__ import annotations

import json
import os
import re
import sys

SCRIPT_DIR = os.path.dirname(os.path.abspath(__file__))
REPO_ROOT = os.path.dirname(os.path.dirname(SCRIPT_DIR))
sys.path.insert(0, SCRIPT_DIR)
import card_io  # noqa: E402

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


def normalize_name(value: str) -> str:
    """Normalize punctuation in a card name to ASCII (typographic apostrophes and
    non-breaking spaces); proper letter accents (é, ü, …) are kept as printed."""
    return value.replace("\u2019", "'").replace("\u2018", "'").replace("\u00a0", " ")


def card_to_dict(card, skipped):
    """Project a Lorcast card onto our structured fields (or None to skip). DSL
    abilities are not emitted — they are authored later by an AI pass."""
    name = card.get("name")
    version = card.get("version")
    full_name = normalize_name(f"{name} - {version}" if version else name)
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
    if kind == "Character" and (strength is None or willpower is None or lore is None):
        skipped.append((full_name, "Character missing strength/willpower/lore"))
        return None
    if kind == "Location" and (willpower is None or move_cost is None):
        skipped.append((full_name, "Location missing willpower/move_cost"))
        return None
    if kind == "Location" and lore is None:
        lore = 0

    image = (card.get("image_uris") or {}).get("digital") or {}
    image = image.get("large") or image.get("normal") or image.get("small")
    inks = list(card["inks"]) if card.get("inks") else ([card["ink"]] if card.get("ink") else [])
    text = (card.get("text") or "").strip()
    out = {
        "name": full_name,
        "type": kind,
        "cost": int(cost),
        "ink": inks,
        "image": image,
        "inkwell": bool(card.get("inkwell")),
        "classifications": card.get("classifications") or [],
        "keywords": map_keywords(card),
        "text": text,
        "collector_number": str(card.get("collector_number")),
    }
    if kind == "Character":
        out.update(strength=int(strength), willpower=int(willpower), lore=int(lore))
    elif kind == "Location":
        out.update(move_cost=int(move_cost), willpower=int(willpower), lore=int(lore))
    m = re.search(r"(?:up to|only have) (\d+) copies of .* in your deck", re.sub(r"\s+", " ", text), re.I)
    if m:
        out["max_copies"] = int(m.group(1))
    return out


def main(argv):
    if len(argv) != 2:
        print(f"usage: {argv[0]} <lorcast_json_path>", file=sys.stderr)
        return 2
    with open(argv[1], "r", encoding="utf-8") as fh:
        cards = json.load(fh)

    skipped = []
    total = 0
    for card in cards:
        code = card.get("set", {}).get("code")
        number = card.get("collector_number")
        if code is None or number is None:
            continue
        rec = card_to_dict(card, skipped)
        if rec is None:
            continue
        out_dir = os.path.join(REPO_ROOT, "cards", code.lower())
        os.makedirs(out_dir, exist_ok=True)
        body = card_io.emit_card(rec, top_level=True)
        with open(os.path.join(out_dir, f"{number}.toml"), "w", encoding="utf-8") as fh:
            fh.write(body + "\n")
        total += 1

    print(f"wrote {total} per-card files under cards/<set>/")
    print(f"skipped {len(skipped)} cards")
    for name, reason in skipped:
        print(f"  - {name}: {reason}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
