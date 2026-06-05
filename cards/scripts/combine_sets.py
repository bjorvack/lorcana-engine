#!/usr/bin/env python3
"""Combine the per-card files ``cards/{set}/{collector}.toml`` into the
``cards/sets/{set}.toml`` files the engine loads (and the wasm crate embeds via
``include_dir!``). The per-card files are the source of truth; these combined
files are a **generated, git-ignored** artifact regenerated here before building
or testing.

Each per-card file already holds one card's fields at the top level, so combining
is purely textual: prepend a ``[[card]]`` header to each (cards ordered by name,
then collector number) under the standard set header. No TOML parser needed, so
this runs on any Python 3.

Usage: python3 cards/scripts/combine_sets.py
"""
import os
import re
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import card_io  # noqa: E402

REPO = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
SETS_DIR = os.path.join(REPO, "cards", "sets")
NAME = re.compile(r'^name\s*=\s*(".*")\s*$', re.M)
# Re-nest a per-card file's top-level sub-tables under `card.` so the combined
# document is a `[[card]]` array (e.g. `[[abilities]]` -> `[[card.abilities]]`,
# `[activated.cost]` -> `[card.activated.cost]`). A header line starts with `[`;
# scalar fields (`name = …`) and value arrays (`ink = […]`) never do.
HEADER = re.compile(r"^(\[\[?)(?!card[.\]])", re.M)


def nest(body: str) -> str:
    """Prefix every sub-table header in a per-card body with `card.`."""
    return HEADER.sub(r"\1card.", body)


def set_dirs():
    """Yield (stem, dir) for each per-card set directory under cards/."""
    root = os.path.join(REPO, "cards")
    for name in sorted(os.listdir(root)):
        d = os.path.join(root, name)
        if name in ("sets", "scripts") or not os.path.isdir(d):
            continue
        if any(f.endswith(".toml") for f in os.listdir(d)):
            yield name, d


def main() -> int:
    os.makedirs(SETS_DIR, exist_ok=True)
    total = 0
    for stem, d in set_dirs():
        cards = []
        for fname in os.listdir(d):
            if not fname.endswith(".toml"):
                continue
            body = open(os.path.join(d, fname), encoding="utf-8").read().rstrip("\n")
            m = NAME.search(body)
            name = m.group(1) if m else fname
            number = int(fname[:-5]) if fname[:-5].isdigit() else 0
            cards.append((name, number, body))
        cards.sort(key=lambda c: (c[0], c[1]))
        blocks = [f"[[card]]\n{nest(body)}" for _, _, body in cards]
        with open(os.path.join(SETS_DIR, f"{stem}.toml"), "w", encoding="utf-8") as fh:
            fh.write(card_io.set_header(stem.upper()))
            fh.write("\n\n".join(blocks))
            fh.write("\n")
        total += len(blocks)
        print(f"set {stem}: combined {len(blocks)} cards")
    print(f"\ncombined {total} cards into {SETS_DIR}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
