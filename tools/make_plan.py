#!/usr/bin/env python3
"""Make a very-normal plan from your own typing.

By default it reads what you typed to Claude Code (never Claude's replies or
tool output). Pass --text to use any other text you wrote instead: notes,
emails, a blog, chat exports. Then it optimises a keyboard layout for it,
splits the move from QWERTY into small stages, picks drill words from your own vocabulary, and writes:

  - a plan file for very-normal (keep it private: the drill words are yours)
  - a summary you can share: your layout, how it scores, and how QWERTY,
    Colemak-DH, Norman and Very Normal score on your typing. No words, no text.

Needs Python 3.11+ and numpy:  uv run --with numpy tools/make_plan.py --name "My Layout"

The corpus never leaves your machine and isn't written anywhere unless you
pass --corpus-out.
"""

import argparse
import collections
import glob
import json
import math
import os
import random
import re
import sys

try:
    import numpy as np
except ImportError:
    sys.exit("make_plan.py needs numpy: run it with `uv run --with numpy tools/make_plan.py`")

QWERTY = "qwertyuiopasdfghjkl;zxcvbnm,./"
CHARS = "abcdefghijklmnopqrstuvwxyz;,./"
REFERENCE = {
    "QWERTY": QWERTY,
    "Norman": "qwdfkjurl;asetgynioh zxcvbpm,./".replace(" ", ""),
    "Colemak-DH": "qwfpbjluy;arstgmneiozxcdvkh,./",
    "Very Normal (Mark's)": "qwydgbhaujilstmpnoerzxcvk;f,./",
}
FINGER = [0, 1, 2, 3, 3, 6, 6, 7, 8, 9]  # column -> finger; 0-3 left, 6-9 right
EFFORT = [
    [2.2, 1.6, 1.3, 1.4, 2.0, 2.0, 1.4, 1.3, 1.6, 2.2],
    [1.6, 1.2, 1.0, 1.0, 1.6, 1.6, 1.0, 1.0, 1.2, 1.6],
    [2.0, 1.8, 1.7, 1.5, 2.2, 2.2, 1.5, 1.7, 1.8, 2.0],
]


# ---------------------------------------------------------------- corpus


def typed_messages(projects):
    """The messages you typed: user turns with plain text, minus pastes,
    slash commands, interruptions and anything long enough to be pasted."""
    out = []
    for path in glob.glob(os.path.join(projects, "**", "*.jsonl"), recursive=True):
        if "/subagents/" in path:
            continue
        with open(path, errors="ignore") as f:
            for line in f:
                try:
                    o = json.loads(line)
                except ValueError:
                    continue
                if o.get("type") != "user" or o.get("isMeta") or o.get("isSidechain"):
                    continue
                c = o.get("message", {}).get("content")
                if isinstance(c, list):
                    c = " ".join(x.get("text", "") for x in c if isinstance(x, dict) and x.get("type") == "text")
                if not isinstance(c, str):
                    continue
                c = re.sub(r"<pasted_content.*?</pasted_content[^>]*>", "", c, flags=re.S).strip()
                if not c or c.startswith("<") or c.startswith("[Request interrupted") or "Caveat:" in c[:20]:
                    continue
                if len(c) > 2000:
                    continue
                out.append(c)
    return out


def text_messages(paths):
    """Plain text from files (or `-` for stdin), split into paragraphs."""
    out = []
    for path in paths:
        if path == "-":
            body = sys.stdin.read()
        else:
            with open(path, errors="ignore") as f:
                body = f.read()
        out += [p.strip() for p in re.split(r"\n\s*\n", body) if p.strip()]
    return out


def counts(text):
    idx = {c: i for i, c in enumerate(CHARS)}
    uni = np.zeros(30)
    bi = np.zeros((30, 30))
    prev = None
    for ch in text:
        i = idx.get(ch)
        if i is None:
            prev = None
            continue
        uni[i] += 1
        if prev is not None and prev != i:
            bi[prev, i] += 1
        prev = i
    return uni, bi


# ---------------------------------------------------------------- scoring

POS = [(i // 10, i % 10) for i in range(30)]
E = np.array([EFFORT[r][c] for r, c in POS])
C = np.zeros((30, 30))
for p, (r1, c1) in enumerate(POS):
    for q, (r2, c2) in enumerate(POS):
        if p == q:
            continue
        f1, f2 = FINGER[c1], FINGER[c2]
        if f1 == f2:  # same finger, different key: the slow, clunky pairs
            C[p, q] = 6 * (1 + 0.5 * abs(r1 - r2)) + (1 if c1 != c2 else 0)
        elif (f1 < 5) == (f2 < 5):
            if abs(f1 - f2) == 1 and abs(r1 - r2) == 2:  # scissor
                C[p, q] = 2
            elif r1 == r2:  # a roll along one row: inward is nicer
                C[p, q] = -0.4 if (f2 > f1 if f1 < 5 else f2 < f1) else -0.2


def positions(layout):
    return np.array([layout.index(c) for c in CHARS])


def cost(P, ub):
    uni, bi = ub
    return (uni @ E[P] + (bi * C[np.ix_(P, P)]).sum()) / uni.sum()


def metrics(layout, ub):
    uni, bi = ub
    P = positions(layout)
    total = uni.sum()
    rows = np.array([POS[p][0] for p in P])
    fing = np.array([FINGER[POS[p][1]] for p in P])
    same = fing[:, None] == fing[None, :]
    return {
        "score": cost(P, ub),
        "home": 100 * uni[rows == 1].sum() / total,
        "sfb": 100 * (bi * same).sum() / max(bi.sum(), 1),
        "pinky": 100 * uni[(fing == 0) | (fing == 9)].sum() / total,
    }


def layout_of(P):
    grid = [" "] * 30
    for i, p in enumerate(P):
        grid[p] = CHARS[i]
    return "".join(grid)


def anneal(ub, locked, iters, rng):
    P = positions(QWERTY)
    free = [i for i, c in enumerate(CHARS) if c not in locked]
    shuffled = P[free].copy()
    rng.shuffle(shuffled)
    P[free] = shuffled
    cur = cost(P, ub)
    best = (cur, P.copy())
    for k in range(iters):
        t = 1.0 * (0.0005 / 1.0) ** (k / iters)
        a, b = rng.sample(free, 2)
        P[a], P[b] = P[b], P[a]
        new = cost(P, ub)
        if new < cur or rng.random() < math.exp((cur - new) / t):
            cur = new
            if cur < best[0]:
                best = (cur, P.copy())
        else:
            P[a], P[b] = P[b], P[a]
    return best


# ---------------------------------------------------------------- stages


def stages(target, freq, per_stage):
    """Swap one letter into its final place at a time, most-typed letters
    first, and group the swaps into stages."""
    state = list(QWERTY)
    swaps = []
    for ch in sorted(CHARS, key=lambda c: -freq[c]):
        want, have = target.index(ch), state.index(ch)
        if want != have:
            other = state[want]
            state[want], state[have] = ch, other
            swaps.append((ch, other))
    assert "".join(state) == target
    out = [QWERTY]
    state = list(QWERTY)
    for i in range(0, len(swaps), per_stage):
        for ch, other in swaps[i : i + per_stage]:
            a, b = state.index(ch), state.index(other)
            state[a], state[b] = state[b], state[a]
        out.append("".join(state))
    return out


def names_and_blurbs(plan_name, ladder, ub):
    """Names a stage after the letters that reach their final key in it."""
    target = ladder[-1]
    names, blurbs = ["Baseline"], ["Your layout as it is today. Record three 60-second real-text runs so later stages have numbers to beat."]
    for s in range(1, len(ladder)):
        before, after = ladder[s - 1], ladder[s]
        moved = sorted({after[i] for i in range(30) if before[i] != after[i] and after[i].isalpha()})
        settled = [c for c in moved if after.index(c) == target.index(c)] or moved
        home = [c for c in settled if after.index(c) // 10 == 1]
        if s == len(ladder) - 1:
            name = f"Fully {plan_name}"
        elif home:
            name = " and ".join(c.upper() for c in home[:2]) + (" come home" if len(home[:2]) > 1 else " comes home")
        else:
            name = " and ".join(c.upper() for c in settled[:2]) + (" settle" if len(settled[:2]) > 1 else " settles")
        m = metrics(after, ub)
        names.append(name)
        blurbs.append(
            f"{', '.join(c.upper() for c in moved)} move. {m['home']:.0f}% of your typing lands on the home row "
            f"and {m['sfb']:.1f}% of letter pairs use the same finger."
        )
    return names, blurbs


# ---------------------------------------------------------------- words


def dictionary():
    """Lowercase dictionary words, minus anything that is mostly a person's name."""
    words, names, small = set(), set(), set()
    for path in ("/usr/share/dict/web2", "/usr/share/dict/words"):
        if os.path.exists(path):
            with open(path) as f:
                words = {w.strip() for w in f if w[:1].islower()}
            break
    if os.path.exists("/usr/share/dict/propernames"):
        with open("/usr/share/dict/propernames") as f:
            names = {w.strip().lower() for w in f}
    if os.path.exists("/usr/share/dict/connectives"):
        with open("/usr/share/dict/connectives") as f:
            small = {w.strip().lower() for w in f}
    return words, names - small


def vocabulary(text, words, names):
    toks = re.findall(r"[a-z]+(?:'[a-z]+)?", text)
    c = collections.Counter(toks)

    def ok(w):
        base, _, tail = w.partition("'")
        if base in names or (words and base not in words):
            return False
        if tail and tail not in ("s", "t", "d", "ll", "re", "ve", "m"):
            return False
        return len(w) > 1 or w in ("a", "i")

    return [(w, n) for w, n in c.most_common() if ok(w) and n >= 3]


def stage_words(vocab, focus):
    scored = [
        (sum(ch in focus for ch in w) / len(w) ** 0.5 * min(n, 50) ** 0.3, w)
        for w, n in vocab
        if any(ch in focus for ch in w) and len(w) >= 3 and n >= 4
    ]
    scored.sort(reverse=True)
    return [w for _, w in scored[:80]]


# ---------------------------------------------------------------- output


def toml_list(items, indent="  "):
    lines, line = [], indent
    for x in items:
        piece = json.dumps(x, ensure_ascii=False) + ", "
        if len(line) + len(piece) > 88:
            lines.append(line.rstrip())
            line = indent
        line += piece
    lines.append(line.rstrip().rstrip(","))
    return "[\n" + "\n".join(lines) + "\n]"


def rows(layout):
    return f"{layout[:10]} {layout[10:20]} {layout[20:]}"


def pretty(layout):
    r = [layout[i : i + 10] for i in (0, 10, 20)]
    return "\n".join(" ".join(x[:5]) + "   " + " ".join(x[5:]) for x in r)


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--name", required=True, help="what to call your layout")
    ap.add_argument("--projects", default=os.path.expanduser("~/.claude/projects"), help="Claude Code transcripts folder")
    ap.add_argument("--text", nargs="+", metavar="FILE", help="use these text files (or - for stdin) instead of Claude Code transcripts")
    ap.add_argument("--lock", default="qwzxcv,./", help="keys that stay where QWERTY has them (default keeps Mac shortcuts)")
    ap.add_argument("--oryx-layout", default="", help="your Oryx layout id, if you have a ZSA board")
    ap.add_argument("--home-row-mods", default="", help='ten comma-separated holds, e.g. ",ctrl,alt,cmd,,,cmd,alt,ctrl,"')
    ap.add_argument("--per-stage", type=int, default=2, help="swaps per stage (each swap moves two keys)")
    ap.add_argument("--restarts", type=int, default=10)
    ap.add_argument("--iters", type=int, default=120000)
    ap.add_argument("--seed", type=int, default=7)
    ap.add_argument("--out", default="plan.toml")
    ap.add_argument("--summary", default="summary.md")
    ap.add_argument("--corpus-out", help="also save the extracted text here (keep it private)")
    args = ap.parse_args()

    msgs = text_messages(args.text) if args.text else typed_messages(args.projects)
    source = ", ".join(args.text) if args.text else args.projects
    if len(msgs) < 200:
        sys.exit(f"only {len(msgs)} messages or paragraphs found in {source}; that's too few to optimise for")
    raw = "\n".join(msgs)
    letters = sum(ch.isalpha() for ch in raw)
    upper = 100 * sum(ch.isupper() for ch in raw) / max(letters, 1)
    text = raw.lower()
    if args.corpus_out:
        with open(args.corpus_out, "w") as f:
            f.write(raw)
    print(f"{len(msgs)} messages, {len(raw)} characters, {upper:.1f}% uppercase", file=sys.stderr)

    rng = random.Random(args.seed)
    np.random.seed(args.seed)
    shuffled = msgs[:]
    rng.shuffle(shuffled)
    cut = int(len(shuffled) * 0.8)
    train = counts("\n".join(shuffled[:cut]).lower())
    test = counts("\n".join(shuffled[cut:]).lower())
    full = counts(text)

    best = None
    for r in range(args.restarts):
        c, P = anneal(train, set(args.lock), args.iters, rng)
        print(f"restart {r + 1}/{args.restarts}: {c:.4f}  {rows(layout_of(P))}", file=sys.stderr)
        if best is None or c < best[0]:
            best = (c, P)
    target = layout_of(best[1])

    freq = collections.Counter(ch for ch in text if ch in CHARS)
    ladder = stages(target, freq, args.per_stage)
    names, blurbs = names_and_blurbs(args.name, ladder, full)
    words, names_dict = dictionary()
    vocab = vocabulary(text, words, names_dict)
    common = [w for w, _ in vocab[:500]]

    mods = [m.strip() for m in args.home_row_mods.split(",")] if args.home_row_mods else []
    if mods and len(mods) != 10:
        sys.exit("--home-row-mods needs ten comma-separated entries")

    out = [
        f"# A very-normal plan for {args.name}, made by tools/make_plan.py from your own",
        "# Claude Code messages. The drill words are your vocabulary: keep this file private.",
        "",
        f"name = {json.dumps(args.name)}",
    ]
    if args.oryx_layout:
        out.append(f"oryx_layout = {json.dumps(args.oryx_layout)}")
    else:
        out.append('# oryx_layout = "abc12"   # your Oryx layout id, if you have a ZSA board')
    if mods:
        out.append(f"home_row_mods = {json.dumps(mods)}")
    else:
        out.append('# home_row_mods = ["", "ctrl", "alt", "cmd", "", "", "cmd", "alt", "ctrl", ""]')
    out += ["", f"common_words = {toml_list(common)}"]
    for s, keys in enumerate(ladder):
        m = metrics(keys, full)
        before = ladder[s - 1] if s else keys
        focus = sorted({keys[i] for i in range(30) if before[i] != keys[i]})
        out += [
            "",
            "[[stage]]",
            f"name = {json.dumps(names[s])}",
            f"blurb = {json.dumps(blurbs[s])}",
            f"keys = {json.dumps(rows(keys))}",
            f"home = {m['home']:.0f}",
            f"same_finger = {m['sfb']:.1f}",
            f"vim = {json.dumps(focus)}",
        ]
        if s:
            out.append(f"words = {toml_list(stage_words(vocab, set(focus)))}")
    with open(args.out, "w") as f:
        f.write("\n".join(out) + "\n")

    table = ["| Layout | Effort (lower is better) | Home row | Same-finger pairs | Pinky |", "|---|---|---|---|---|"]
    for label, lay in [(args.name, target)] + list(REFERENCE.items()):
        m = metrics(lay, test)
        table.append(f"| {label} | {m['score']:.3f} | {m['home']:.0f}% | {m['sfb']:.1f}% | {m['pinky']:.1f}% |")
    top = "".join(c for c, _ in freq.most_common(10))
    q = metrics(QWERTY, full)
    summary = "\n".join(
        [
            f"# {args.name}",
            "",
            "```",
            pretty(target),
            "```",
            "",
            f"- Typing studied: {len(msgs)} messages, {len(raw):,} characters, {upper:.1f}% uppercase",
            f"- Ten most-typed keys: `{top}`",
            f"- Locked in place: `{args.lock}`",
            f"- Stages from QWERTY: {len(ladder) - 1}, moving {sum(1 for i in range(30) if target[i] != QWERTY[i])} keys",
            f"- On QWERTY: {q['home']:.0f}% home row, {q['sfb']:.1f}% same-finger pairs",
            "",
            "Scored on 20% of messages the optimiser never saw:",
            "",
            *table,
            "",
            "Made with [very-normal](https://github.com/markryall/very-normal)'s tools/make_plan.py.",
        ]
    )
    with open(args.summary, "w") as f:
        f.write(summary + "\n")
    print(summary)
    print(f"\nwrote {args.out} (private) and {args.summary} (shareable)", file=sys.stderr)


if __name__ == "__main__":
    main()
