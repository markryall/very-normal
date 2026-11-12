# Make your own layout

Very Normal was optimised on one person's typing. Yours is probably a bit
different. This repo can build a layout from what *you* type to Claude Code,
split the move from QWERTY into small stages, and drill you through them.

Clone the repo, open Claude Code in it, and paste the prompt below.

---

## The prompt

> Help me make my own keyboard layout with this repo's `tools/make_plan.py`,
> following the steps below. Talk me through it as you go.
>
> **Privacy rules for the whole task.** My Claude Code transcripts are
> private. Never print, quote or summarise my messages. Never put the plan
> file or any extracted text anywhere public or into git. The only thing that
> may be shared is `summary.md`, and only if I say so.
>
> 1. **Check the ingredients.** Confirm `python3` can import `numpy` (if not,
>    use `uv run --with numpy`, or ask me how I'd like to install it). Count
>    the `*.jsonl` files under `~/.claude/projects`. The tool needs at least a
>    few hundred typed messages; if there are fewer, tell me and stop.
>
> 2. **Ask me, in one go:**
>    - What to call the layout. Suggest a few names. A fun trick is a word
>      that can be typed using only its home row, but you can only know
>      that after the run, so offer to rename it then.
>    - Which keys must not move. The default `qwzxcv,./` keeps Cmd+Q, W, Z,
>      X, C and V where they are. Mention that every locked key makes the
>      layout a little worse, and that locking nothing is fine too.
>    - Whether I have a ZSA keyboard with an Oryx layout. If I do, ask for
>      its public link. Then read the home row holds from Oryx yourself
>      (POST a `layout(hashId, geometry, revisionId: "latest")` query to
>      `https://oryx.zsa.io/graphql`, as `src/oryx.rs` does). Turn them into
>      `--home-row-mods` and show me what you found.
>
> 3. **Run it.** All of it stays on this machine:
>
>    ```sh
>    mkdir -p tmp
>    python3 tools/make_plan.py --name "<name>" --lock "<keys>" \
>      [--oryx-layout <id>] [--home-row-mods "<ten comma-separated holds>"] \
>      --out tmp/plan.toml --summary tmp/summary.md
>    ```
>
>    It takes a minute or two. Its six restarts usually land on slightly
>    different layouts that score almost the same. That's normal: many
>    layouts tie for best.
>
> 4. **Check the drill words for names.** The tool drops dictionary proper
>    names, but read every `words` and `common_words` entry in
>    `tmp/plan.toml` yourself. Remove people's names, company and customer
>    names, and anything else I wouldn't want on screen while a colleague
>    looks over my shoulder. Tell me how many you removed, but not which ones.
>
> 5. **Tell me honestly how it did.** Show me the layout and the table from
>    `tmp/summary.md`. Compare my layout with Colemak-DH in plain words: if
>    mine only wins by a few percent, say so. In that case a well-known
>    layout with tutorials and a community might be the better choice, and I
>    should decide knowing that.
>
> 6. **Install the plan** if I want it: copy `tmp/plan.toml` to
>    `~/.config/very-normal/plan.toml`, build with `cargo install --path .`,
>    and run `very-normal plan` to show the stages. If I have an Oryx layout,
>    run `very-normal next` to show the stage 1 edits.
>
> 7. **Offer to share the summary.** `tmp/summary.md` holds no words or text,
>    just my layout and its scores. If I'm happy to, help me open an issue on
>    `markryall/very-normal` titled `Layout: <name>` with the summary as the
>    body. The author is collecting them to see how much layouts vary
>    between people. Only do this if I say yes.

---

## What the tool does

`tools/make_plan.py` (Python 3.11+, numpy):

1. **Reads what you typed.** It takes your user messages from
   `~/.claude/projects/**/*.jsonl`. It skips Claude's replies, tool output,
   pasted blocks, slash commands and anything over 2,000 characters, since
   that was probably pasted rather than typed.
2. **Scores layouts on your typing.** Each key has an effort cost (home row
   cheap, pinky stretches dear). It penalises *same-finger pairs* (two
   letters in a row on one finger) heavily, and *scissors* (adjacent fingers
   jumping between top and bottom rows) lightly. It slightly rewards rolls
   along a row. All three rows are scored for an ortholinear board; the
   quote key and outer columns stay put.
3. **Searches with simulated annealing** from QWERTY, with your locked keys
   pinned. It trains on 80% of your messages and scores on the other 20%,
   so the table shows how the layout does on typing it never saw.
4. **Builds the ladder.** It swaps one letter into its final key at a time,
   most-typed letters first, and groups the swaps two to a stage. The home
   row fills up in the first few stages.
5. **Picks drill words** from your own vocabulary that use each stage's
   moved keys. It filters them through the system dictionary and drops
   proper names.

Finding the true best layout is a quadratic assignment problem (NP-hard),
and "best" depends on how the scoring is weighted anyway. Treat the result
as a very good layout for you, not the one true answer.
