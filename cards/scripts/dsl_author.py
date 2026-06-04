#!/usr/bin/env python3
"""Local-LLM card-ability authoring + benchmarking for the Lorcana engine.

The remote agent (Devin) should *not* hand-author cards one-by-one — that burns
tokens. Instead this script offloads drafting to a local Ollama model and uses the
engine's own DSL parser (`validate_card` binary) as the correctness gate. Cards
that don't parse are **skipped**, never silently accepted.

Two modes:

  benchmark  Run the held-out ground-truth (text -> DSL) pairs mined from the
             committed card files through one or more models; report parse-valid
             rate, semantic AST-match rate vs ground truth, and latency. Use this
             to pick a model.

  author     Draft abilities for cards that have rules `text` but no authored
             abilities yet; write the ones that validate to an output TOML and log
             the skips. Designed to run unattended (e.g. in the background).

The DSL *reference* in the prompt is generated from the engine source on every run
(`build_reference`), so it never drifts from `dsl.rs` / `loader.rs`.
"""

from __future__ import annotations

import argparse
import glob
import json
import os
import random
import re
import subprocess
import sys
import time
import tomllib
import urllib.request
from dataclasses import dataclass, field
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
DSL_RS = REPO / "src/domain/cards/dsl.rs"
LOADER_RS = REPO / "src/domain/cards/loader.rs"
SETS_DIR = REPO / "cards/sets"
VALIDATOR = REPO / "target/debug/validate_card"
OLLAMA_URL = "http://localhost:11434/api/generate"


# --------------------------------------------------------------------------- #
# 1. Build the DSL reference from the engine source (kept in sync, not hardcoded)
# --------------------------------------------------------------------------- #
def _slice(src: str, start_marker: str) -> str:
    """Return the body of the fn whose signature contains `start_marker`."""
    i = src.find(start_marker)
    if i < 0:
        return ""
    # crude brace-matched body
    depth = 0
    started = False
    out = []
    for ch in src[i:]:
        if ch == "{":
            depth += 1
            started = True
        out.append(ch)
        if ch == "}":
            depth -= 1
            if started and depth == 0:
                break
    return "".join(out)


def extract_vocab() -> dict:
    dsl = DSL_RS.read_text()
    loader = LOADER_RS.read_text()

    trig_body = _slice(dsl, "fn trigger_from")
    # quoted strings on the LHS of `=>` lines (skip the `other` catch-all)
    triggers = sorted({
        m for line in trig_body.splitlines() if "=>" in line and "other" not in line
        for m in re.findall(r'"([a-z_]+)"', line.split("=>")[0])
    })

    verb_body = _slice(dsl, "fn effect_from_table")
    verbs = sorted(set(
        re.findall(r'contains_key\("([a-z_]+)"\)', verb_body)
        + re.findall(r'\.get\("([a-z_]+)"\)', verb_body)
    ))

    restr_body = _slice(dsl, "fn restriction_from")
    restrictions = sorted({
        m for line in restr_body.splitlines() if "=>" in line and "other" not in line
        for m in re.findall(r'"([a-z_]+)"', line.split("=>")[0])
    })

    kw_body = _slice(loader, "fn keyword_from")
    keywords = sorted({
        m for line in kw_body.splitlines() if "=>" in line and "other" not in line
        for m in re.findall(r'"([A-Za-z]+)"', line.split("=>")[0])
    })

    return {
        "triggers": triggers,
        "verbs": verbs,
        "restrictions": restrictions,
        "keywords": keywords,
    }


# --------------------------------------------------------------------------- #
# 2. Mine real (text, DSL-block) pairs from the committed card files
# --------------------------------------------------------------------------- #
@dataclass
class Pair:
    name: str
    text: str
    header_toml: str  # the [[card]] block WITHOUT its abilities (parse context)
    abilities_toml: str  # the verbatim [[card.abilities]] block(s)


def _card_blocks(path: Path):
    """Yield (raw_block_text) for each [[card]] in a set file."""
    lines = path.read_text().splitlines(keepends=True)
    cur, started = [], False
    for ln in lines:
        if ln.strip() == "[[card]]":
            if started:
                yield "".join(cur)
                cur = []
            started = True
        if started:
            cur.append(ln)
    if started and cur:
        yield "".join(cur)


def mine_pairs() -> list[Pair]:
    pairs: list[Pair] = []
    for f in sorted(glob.glob(str(SETS_DIR / "*.toml"))):
        path = Path(f)
        # parsed view to filter to pure triggered-ability cards
        data = tomllib.load(open(f, "rb"))
        pure_names = {
            c["name"]
            for c in data.get("card", [])
            if c.get("text") and "abilities" in c
            and "activated" not in c and "statics" not in c
        }
        for block in _card_blocks(path):
            name_m = re.search(r'name\s*=\s*"([^"]+)"', block)
            if not name_m or name_m.group(1) not in pure_names:
                continue
            text_m = re.search(r'^text\s*=\s*"(.*)"\s*$', block, re.M)
            ab_i = block.find("[[card.abilities]]")
            if not text_m or ab_i < 0:
                continue
            header = block[:ab_i].rstrip() + "\n"
            abilities = block[ab_i:].rstrip() + "\n"
            text = text_m.group(1).encode().decode("unicode_escape")
            pairs.append(Pair(name_m.group(1), text, header, abilities))
    return pairs


# --------------------------------------------------------------------------- #
# 3. Prompt assembly
# --------------------------------------------------------------------------- #
def build_reference(vocab: dict, examples: list[Pair]) -> str:
    ex = "\n\n".join(
        f'Card: "{p.text}"\n{p.abilities_toml.strip()}' for p in examples
    )
    return f"""You convert Lorcana card text into the engine's TOML ability DSL.
Output ONLY [[card.abilities]] TOML block(s). No prose, no code fences.

TRIGGERS (the `on` field): {", ".join(vocab["triggers"])}
EFFECT VERBS (keys inside `do = {{ ... }}`): {", ".join(vocab["verbs"])}
RESTRICTIONS (restrict = "..."): {", ".join(vocab["restrictions"])}
KEYWORDS (grant_keyword = "..."): {", ".join(vocab["keywords"])}

Rules:
- Optional ("you may ..."): add a line `may = true`.
- Multiple effects: `do = [ {{ ... }}, {{ ... }} ]`.
- Targets are strings: "self", "chosen character", "chosen opposing character",
  "another chosen character", "all opposing characters", "your other characters",
  "chosen item", "character named X", "chosen character with cost N or less".
- Use give_strength with a negative number for "-N {{S}}".
- Do NOT invent keys that aren't listed above. If the text only describes a keyword
  reminder (e.g. plain "Rush"/"Evasive") output NOTHING.

EXAMPLES:

{ex}

Now convert this card. Output ONLY the TOML block:
Card: """


# --------------------------------------------------------------------------- #
# 4. Model call + validation gate
# --------------------------------------------------------------------------- #
def call_model(model: str, prompt: str, keep_alive: str = "30m",
               temperature: float = 0.0, timeout: int = 120,
               think: bool = False) -> tuple[str, float]:
    body = json.dumps({
        "model": model,
        "prompt": prompt,
        "stream": False,
        "keep_alive": keep_alive,
        # Reasoning models (qwen3, etc.): `think=True` lets them reason first; the
        # reasoning is emitted inline, and `clean_dsl` keeps only the DSL block.
        "think": think,
        "options": {"temperature": temperature},
    }).encode()
    t0 = time.time()
    req = urllib.request.Request(OLLAMA_URL, data=body,
                                 headers={"Content-Type": "application/json"})
    with urllib.request.urlopen(req, timeout=timeout) as r:
        out = json.load(r)
    return out.get("response", ""), time.time() - t0


_FENCE = re.compile(r"```(?:toml)?\s*(.*?)```", re.S)


def clean_dsl(raw: str) -> str:
    """Strip code fences / stray prose; keep from the first [[card.abilities]]."""
    m = _FENCE.search(raw)
    body = m.group(1) if m else raw
    i = body.find("[[card.abilities]]")
    return body[i:].strip() if i >= 0 else body.strip()


def validate(toml_text: str, debug: bool = False) -> tuple[bool, str]:
    args = [str(VALIDATOR)] + (["--debug"] if debug else [])
    p = subprocess.run(args, input=toml_text, capture_output=True, text=True)
    return p.returncode == 0, (p.stdout + p.stderr).strip()


# --------------------------------------------------------------------------- #
# 5. Benchmark mode
# --------------------------------------------------------------------------- #
def run_benchmark(models: list[str], n_eval: int, n_shot: int, seed: int,
                  timeout: int = 120, think: bool = False) -> None:
    vocab = extract_vocab()
    pairs = mine_pairs()
    random.seed(seed)
    random.shuffle(pairs)
    shots, rest = pairs[:n_shot], pairs[n_shot:]
    eval_set = rest[:n_eval]
    reference = build_reference(vocab, shots)
    print(f"vocab: {len(vocab['triggers'])} triggers, {len(vocab['verbs'])} verbs, "
          f"{len(vocab['keywords'])} keywords")
    print(f"few-shot: {len(shots)}  |  eval: {len(eval_set)}\n")

    for model in models:
        parsed = matched = 0
        total_t = 0.0
        for p in eval_set:
            raw, dt = call_model(model, reference + f'"{p.text}"',
                                 timeout=timeout, think=think)
            total_t += dt
            dsl = clean_dsl(raw)
            model_card = p.header_toml + dsl + "\n"
            ok, ast_model = validate(model_card, debug=True)
            if not ok:
                continue
            parsed += 1
            _, ast_gt = validate(p.header_toml + p.abilities_toml, debug=True)
            if ast_model == ast_gt:
                matched += 1
        n = len(eval_set)
        print(f"{model:24s}  parse-valid {parsed}/{n} ({parsed/n:.0%})  "
              f"AST-match {matched}/{n} ({matched/n:.0%})  "
              f"avg {total_t/n:.2f}s/card")


# --------------------------------------------------------------------------- #
# 6. Author mode (unattended; skips failures)
# --------------------------------------------------------------------------- #
def candidate_targets() -> list[dict]:
    """Cards with rules text but no authored abilities/activated/statics."""
    out = []
    for f in sorted(glob.glob(str(SETS_DIR / "*.toml"))):
        data = tomllib.load(open(f, "rb"))
        for c in data.get("card", []):
            if (c.get("text") and "abilities" not in c
                    and "activated" not in c and "statics" not in c):
                out.append({"file": Path(f).name, **c})
    return out


def header_for(card: dict) -> str:
    """A minimal but parse-faithful [[card]] header (incl. classifications)."""
    lines = ['[[card]]', f'name = "{card["name"]}"', f'type = "{card.get("type","Character")}"',
             f'cost = {card.get("cost",1)}']
    ink = card.get("ink", ["Amber"])
    lines.append("ink = [" + ", ".join(f'"{i}"' for i in ink) + "]")
    for k in ("strength", "willpower", "lore"):
        if k in card:
            lines.append(f"{k} = {card[k]}")
    if card.get("classifications"):
        cl = ", ".join(f'"{c}"' for c in card["classifications"])
        lines.append(f"classifications = [{cl}]")
    return "\n".join(lines) + "\n"


def run_author(model: str, n_shot: int, seed: int, limit: int,
               out_path: Path, skip_path: Path) -> None:
    vocab = extract_vocab()
    pairs = mine_pairs()
    random.seed(seed)
    random.shuffle(pairs)
    reference = build_reference(vocab, pairs[:n_shot])
    targets = candidate_targets()
    if limit:
        targets = targets[:limit]

    authored = skipped = 0
    out_f = open(out_path, "w")
    skip_f = open(skip_path, "w")
    out_f.write("# Auto-drafted abilities (model + engine-validated). Review before committing.\n")
    for i, card in enumerate(targets, 1):
        try:
            raw, _ = call_model(model, reference + f'"{card["text"]}"')
        except Exception as e:  # network/model hiccup -> skip
            skip_f.write(f'{card["name"]}\tMODEL_ERROR\t{e}\n')
            skipped += 1
            continue
        dsl = clean_dsl(raw)
        if "[[card.abilities]]" not in dsl:
            skip_f.write(f'{card["name"]}\tNO_ABILITY\t{dsl[:80]!r}\n')
            skipped += 1
            continue
        ok, msg = validate(header_for(card) + dsl + "\n")
        if ok:
            out_f.write(f'\n# {card["file"]}  ::  {card["name"]}\n')
            out_f.write(f'#   text: {card["text"]}\n')
            out_f.write(f'# name = "{card["name"]}"\n{dsl}\n')
            out_f.flush()
            authored += 1
        else:
            skip_f.write(f'{card["name"]}\tINVALID\t{msg}\n')
            skip_f.flush()
            skipped += 1
        if i % 25 == 0:
            print(f"[{i}/{len(targets)}] authored={authored} skipped={skipped}", flush=True)
    out_f.close()
    skip_f.close()
    print(f"\nDONE  authored={authored}  skipped={skipped}  -> {out_path}  (skips: {skip_path})")


# --------------------------------------------------------------------------- #
def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__)
    sub = ap.add_subparsers(dest="cmd", required=True)

    b = sub.add_parser("benchmark", help="score models on mined ground-truth pairs")
    b.add_argument("--models", nargs="+",
                   default=["qwen2.5-coder:7b", "qwen2.5-coder:3b"])
    b.add_argument("--eval", type=int, default=40)
    b.add_argument("--shots", type=int, default=6)
    b.add_argument("--seed", type=int, default=7)
    b.add_argument("--timeout", type=int, default=120,
                   help="per-card request timeout (s); raise for reasoning models")
    b.add_argument("--think", action="store_true",
                   help="let reasoning models reason first (qwen3, etc.)")

    a = sub.add_parser("author", help="draft + validate abilities for unauthored cards")
    a.add_argument("--model", default="qwen2.5-coder:7b")
    a.add_argument("--shots", type=int, default=6)
    a.add_argument("--seed", type=int, default=7)
    a.add_argument("--limit", type=int, default=0, help="0 = all candidates")
    a.add_argument("--out", default=str(REPO / "cards/scripts/drafted_abilities.toml"))
    a.add_argument("--skips", default=str(REPO / "cards/scripts/drafted_skips.tsv"))

    r = sub.add_parser("reference", help="print the generated DSL reference and exit")
    r.add_argument("--shots", type=int, default=6)
    r.add_argument("--seed", type=int, default=7)

    args = ap.parse_args()
    if not VALIDATOR.exists():
        sys.exit(f"validator not built: {VALIDATOR}\n  run: cargo build --bin validate_card")

    if args.cmd == "benchmark":
        run_benchmark(args.models, args.eval, args.shots, args.seed,
                      timeout=args.timeout, think=args.think)
    elif args.cmd == "author":
        run_author(args.model, args.shots, args.seed, args.limit,
                   Path(args.out), Path(args.skips))
    elif args.cmd == "reference":
        pairs = mine_pairs()
        random.seed(args.seed)
        random.shuffle(pairs)
        print(build_reference(extract_vocab(), pairs[:args.shots]))


if __name__ == "__main__":
    main()
