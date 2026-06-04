#!/usr/bin/env python3
"""Backfill a real `text` field (printed rules text) into each [[card]] from the
research dump, replacing the redundant `# text:` comment. Matched per-set by full
name. Idempotent-ish: if a `text =` field already exists for a card, it's left.

Usage: python3 cards/scripts/backfill_text.py /path/to/all_cards.json
"""
import json, os, re, sys
SETS = os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(__file__))), "sets")

def norm(s):
    return (s or "").replace("\u2019", "'").replace("\u2018", "'").replace("\u00a0", " ")

def full(c):
    return norm(f"{c['name']} - {c['version']}" if c.get("version") else c["name"])

def esc(s):
    return s.replace("\\", "\\\\").replace('"', '\\"').replace("\n", "\\n").replace("\t", "\\t").replace("\r", "")

def main(argv):
    cards = json.load(open(argv[1], encoding="utf-8"))
    by_set = {}
    for c in cards:
        code = (c.get("set") or {}).get("code")
        if code:
            by_set.setdefault(code.lower(), {})[full(c)] = norm(c.get("text") or "").strip()

    total = 0
    for fn in sorted(os.listdir(SETS)):
        if not fn.endswith(".toml"):
            continue
        lookup = by_set.get(fn[:-5], {})
        lines = open(os.path.join(SETS, fn), encoding="utf-8").read().split("\n")
        out, i, cur, changed = [], 0, None, 0
        while i < len(lines):
            ln = lines[i]
            if ln == "[[card]]":
                cur = None
            m = re.match(r'name = "(.*)"$', ln)
            if m and cur is None:
                cur = m.group(1).replace('\\"', '"').replace("\\\\", "\\")
            # Replace the `# text:` comment block with a real `text = "..."` field.
            if ln.strip() == "# text:" and cur is not None and not any(
                l.startswith("text = ") for l in lines[max(0, i - 12):i]
            ):
                # skip the comment block (this line + following `#   ` lines)
                j = i + 1
                while j < len(lines) and lines[j].lstrip().startswith("#"):
                    j += 1
                txt = lookup.get(cur, "")
                if txt:
                    out.append(f'text = "{esc(txt)}"')
                    changed += 1
                i = j
                continue
            out.append(ln)
            i += 1
        if changed:
            open(os.path.join(SETS, fn), "w", encoding="utf-8").write("\n".join(out))
            total += changed
    print(f"added {total} text fields")

if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
