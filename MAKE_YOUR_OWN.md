# Make your own layout

Very Normal was optimised on one person's typing. Yours is probably a bit
different. very-normal can build a layout from what *you* type, split the
move from QWERTY into small stages, and drill you through them.

```sh
cargo install very-normal
very-normal                  # the first run offers to make your layout
very-normal make-plan --name "My Layout" --text ~/notes   # or do it any time
very-normal use my-layout    # switch to it; your progress on each plan is kept
very-normal share            # add your layout to the collection
```

Making a plan never touches the plan you're practising or your progress, so
experiment freely: more text, other locks, other names. Only your very
first plan becomes active by itself.

## Sharing

I'm collecting layouts to see how much they vary between people.
`very-normal share` opens a new issue on
[markryall/very-normal](https://github.com/markryall/very-normal/issues)
titled `Layout: <name>`, with your newest plan's summary from
`~/.config/very-normal/plans/` as its body (`very-normal share <name>` picks
another).
Nothing is posted until you press Submit.

The summary holds no words or text from your writing. It has:

- your layout, and which keys you locked
- how much you studied (messages and characters), and where it came from
  (Claude Code messages, other writing, or both), but not file names
- how often you type each of the 30 keys
- how your layout, QWERTY, Colemak-DH, Norman and Very Normal score on your
  typing
- every restart of the search, with its layout and score, and how many keys
  all of them agreed on

That last part matters for comparisons. Many layouts score almost the same,
so the same typing can land on different layouts from different random
starts, and different locked keys change the result too. The restarts show
how much of the difference between two people is chance.

## Naming it

A fun trick is a name you can type using only your new home row. After a
run, make-plan suggests some from your own vocabulary. Run it again with
`--name "<idea>"`: the same seed gives the same layout.

## What it does

1. **Reads what you typed.** By default it reads your messages from
   `~/.claude/projects/**/*.jsonl`. It skips Claude's replies, tool output,
   subagents, pasted blocks, slash commands and anything over 2,000
   characters, since that was probably pasted rather than typed. `--text`
   adds other writing, split into paragraphs. It needs at least 200 messages
   or paragraphs.
2. **Scores layouts on your typing.** Each key has an effort cost (home row
   cheap, pinky stretches dear). It penalises *same-finger pairs* (two
   letters in a row on one finger) heavily, and *scissors* (adjacent fingers
   jumping between top and bottom rows) lightly. It slightly rewards rolls
   along a row. All three rows are scored for an ortholinear board; the
   quote key and outer columns stay put.
3. **Searches with simulated annealing**, 20 restarts in parallel from
   shuffles of QWERTY, with your locked keys pinned. It trains on 80% of
   your messages and scores on the other 20%, so the summary's table shows
   how the layout does on typing it never saw.
4. **Builds the ladder.** It swaps one letter into its final key at a time,
   most-typed letters first, and groups the swaps two to a stage. The home
   row fills up in the first few stages.
5. **Picks drill words** from your own vocabulary that use each stage's
   moved keys. It keeps only words in the system dictionary, drops the
   dictionary's proper names and any word you mostly write with a capital,
   and keeps words you used at least four times. Names can still slip
   through, so read the `words` lists in `plan.toml` before practising where
   colleagues can see your screen.

Finding the true best layout is a quadratic assignment problem (NP-hard),
and "best" depends on how the scoring is weighted anyway. Treat the result
as a very good layout for you, not the one true answer. If yours only beats
Colemak-DH by a few percent, a well-known layout with tutorials and a
community might suit you better, and the summary's table lets you decide
knowing that.
