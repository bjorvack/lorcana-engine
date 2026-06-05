#!/usr/bin/env python3
"""One-time migration: split the combined ``cards/sets/{set}.toml`` files into
per-card files ``cards/{set}/{collector_number}.toml`` (structured fields + text;
authored DSL abilities are intentionally dropped — they are (re)authored by a
later AI pass).

The collector number for each card is looked up from the public Lorcast API,
matched by the card's image hash (``crd_…``) which is the Lorcast card id — robust
even for reprints that share a name. ~one API request per set.

Usage: python3 cards/scripts/split_sets.py
"""
import json
import os
import re
import subprocess
import sys
import tomllib
import urllib.parse

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import card_io  # noqa: E402

REPO = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
SETS_DIR = os.path.join(REPO, "cards", "sets")
HASH = re.compile(r"(crd_[0-9a-f]+)")


def api_code(stem: str) -> str:
    return stem if stem.isdigit() else stem.upper()


def fetch_numbers(code: str) -> dict:
    """Map ``crd_…`` id -> collector number for every card in the set."""
    url = "https://api.lorcast.com/v0/cards/search"
    q = urllib.parse.urlencode({"q": f"set:{code}"})
    out = subprocess.run(
        ["curl", "-s", "--max-time", "30", f"{url}?{q}"],
        capture_output=True, text=True,
    ).stdout
    res = json.loads(out).get("results", [])
    return {c["id"]: str(c["collector_number"]) for c in res if c.get("id")}


def main() -> int:
    total = 0
    for fname in sorted(os.listdir(SETS_DIR)):
        if not fname.endswith(".toml"):
            continue
        stem = fname[:-5]
        doc = tomllib.load(open(os.path.join(SETS_DIR, fname), "rb"))
        cards = doc.get("card", [])
        numbers = fetch_numbers(api_code(stem))
        out_dir = os.path.join(REPO, "cards", stem)
        os.makedirs(out_dir, exist_ok=True)
        for card in cards:
            m = HASH.search(card.get("image", ""))
            if not m or m.group(1) not in numbers:
                print(f"  MISS: {stem} {card.get('name')!r}", file=sys.stderr)
                return 1
            number = numbers[m.group(1)]
            body = card_io.emit_card(card_io.normalize(card), top_level=True)
            with open(os.path.join(out_dir, f"{number}.toml"), "w", encoding="utf-8") as fh:
                fh.write(body + "\n")
            total += 1
        print(f"set {stem}: wrote {len(cards)} cards")
    print(f"\nwrote {total} per-card files")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
