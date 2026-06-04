#!/usr/bin/env python3
"""Backfill ink / image / max_copies into the committed per-set card TOML,
in place, WITHOUT disturbing hand-authored abilities or the `# text:` comments.

Usage: python3 cards/scripts/backfill_meta.py /path/to/all_cards.json

Matches each [[card]] (by full name, within its set) to the research dump and
inserts `ink`, `image`, and (for the few rule-breakers) `max_copies` right after
the card's `cost` line. Idempotent: skips cards that already have `ink`.
"""
import json, os, re, sys

SCRIPT_DIR = os.path.dirname(os.path.abspath(__file__))
SETS_DIR = os.path.join(os.path.dirname(SCRIPT_DIR), "sets")

def full_name(c):
    return f"{c['name']} - {c['version']}" if c.get("version") else c["name"]

def inks_of(c):
    if c.get("inks"):
        return list(c["inks"])
    return [c["ink"]] if c.get("ink") else []

def image_of(c):
    u = (c.get("image_uris") or {}).get("digital") or {}
    return u.get("large") or u.get("normal") or u.get("small")

def max_copies_of(c):
    t = re.sub(r"\s+", " ", c.get("text") or "")
    m = re.search(r"(?:up to|only have) (\d+) copies of .* in your deck", t, re.I)
    return int(m.group(1)) if m else None

def toml_str(v):
    return '"' + v.replace("\\", "\\\\").replace('"', '\\"') + '"'

def main(argv):
    if len(argv) != 2:
        print(f"usage: {argv[0]} <lorcast_json>", file=sys.stderr); return 2
    cards = json.load(open(argv[1], encoding="utf-8"))
    by_set = {}
    for c in cards:
        code = (c.get("set") or {}).get("code")
        if code:
            by_set.setdefault(code.lower(), {})[full_name(c)] = c

    total_ink = total_img = total_max = files = 0
    for fn in sorted(os.listdir(SETS_DIR)):
        if not fn.endswith(".toml"):
            continue
        code = fn[:-5]
        lookup = by_set.get(code, {})
        path = os.path.join(SETS_DIR, fn)
        lines = open(path, encoding="utf-8").read().split("\n")
        out, i, changed = [], 0, False
        cur_name = None
        while i < len(lines):
            line = lines[i]
            out.append(line)
            if line == "[[card]]":
                cur_name = None
            m = re.match(r'name = "(.*)"$', line)
            if m and cur_name is None:
                cur_name = m.group(1).replace('\\"', '"').replace("\\\\", "\\")
            # Insert right after the cost line (scalar; before sub-tables/comments).
            if re.match(r"cost = \d+$", line) and cur_name is not None:
                # Skip if already backfilled (next non-blank lines contain ink).
                already = any(l.startswith("ink = ") for l in lines[i+1:i+4])
                c = lookup.get(cur_name)
                if c and not already:
                    inks = inks_of(c)
                    if inks:
                        out.append("ink = [" + ", ".join(toml_str(x) for x in inks) + "]")
                        total_ink += 1; changed = True
                    img = image_of(c)
                    if img:
                        out.append(f"image = {toml_str(img)}")
                        total_img += 1; changed = True
                    mc = max_copies_of(c)
                    if mc is not None:
                        out.append(f"max_copies = {mc}")
                        total_max += 1; changed = True
            i += 1
        if changed:
            open(path, "w", encoding="utf-8").write("\n".join(out))
            files += 1
    print(f"updated {files} files: +{total_ink} ink, +{total_img} image, +{total_max} max_copies")
    return 0

if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
