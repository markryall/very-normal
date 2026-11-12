# very-normal

A terminal typing tutor for moving a keyboard from QWERTY to a new layout a
few keys at a time, with a bossy coach and checks against your ZSA Oryx
layout.

It ships with **Very Normal**, a layout optimised on 318k characters of my
own typing to Claude Code, with Q, W, Z, X, C and V locked so every Mac
shortcut stays where it was:

```
q w y d g   b h a u j
i l s t m   p n o e r  '
z x c v k   ; f , . /
```

**Want one made from your typing?** See [MAKE_YOUR_OWN.md](MAKE_YOUR_OWN.md).
It has a prompt that walks Claude Code through building a layout and plan
from your own transcripts, plus a summary you can share. I'm collecting those
to see how much layouts vary between people. It also takes any other text you wrote
(`--text notes.md emails.txt`) instead of transcripts.

## Use

```sh
very-normal                  # the trainer
very-normal status           # what the coach thinks of your practice today
very-normal next             # the Oryx edits for the next stage, and the link to make them
very-normal check            # read the layout from Oryx and compare it with your stage
very-normal plan             # which plan you're on, and its stages
very-normal keymap kanata    # this stage as a kanata config (or karabiner), for any keyboard
very-normal stage 2          # set the stage you're practising
very-normal baseline 35      # type in a baseline instead of recording one
very-normal import backup.json   # merge a backup copied out of the web tutor
very-normal charts           # a page of charts from your progress, opened in the browser
very-normal export           # print progress as JSON
```

In the trainer:

- `←` `→` change stage. `1`–`6` pick a drill: key recall, new keys, review,
  60 seconds of real text, markdown links, or one-handed mirror practice.
- `enter` starts typing. Wrong keys don't advance. If you pause, the key you
  want lights up on the map. `esc` stops without saving.
- `c` checks Oryx and `o` opens the layout there. `g` opens your charts.
- `k`, `v` and `w` tick the gate's manual boxes.

## The coach

- Aim for 20 minutes a day, in short bursts. It nags until you get there and
  calls you out for skipping a day.
- After 15 minutes in a row it tells you to wrap up. After 25, or 45 minutes
  in a day, it won't start a drill unless you press enter twice.
- A five-minute pause counts as a break.
- If a session starts going backwards (accuracy a few points under your best
  earlier in the session, speed 15% down, three gate misses in a row, or
  three restarts in ten minutes), it suggests a break. That's usually
  frustration or tired fingers, not you getting worse.

## Charts

`very-normal charts` (or `g` in the trainer) writes
`~/.local/share/very-normal/charts.html`, one self-contained page that works
offline, and opens it. It shows:

- speed and accuracy run by run, with stages shaded and the gate's lines
- how fast each stage climbs (a trend per stage) and where each stage
  started and got to against your QWERTY baseline
- the board as a heatmap of slow or missed keys, the slowest and most missed
  keys, and each key's speed over time
- what you typed instead of the key you wanted, tagged when your finger went
  to where that key used to be, and the slowest pairs of keys
- a practice calendar, and whether you slow down late in a session or at
  certain times of day

Runs saved before charts existed have no per-run key timings, typos or key
pairs, so those charts fill in as you practise.

## The warm-up

The first drill of the day starts with a guided one-minute warm-up: slow
breaths, then gentle hand and wrist stretches. `enter` moves to the next step,
`s` skips it (the coach will judge you), and `u` runs it any time.

## The gate

Before climbing a stage:

1. New-keys runs at 97%+ accuracy in 3 of your last 5.
2. Real text back to 90% of your baseline speed at 95%+ accuracy.
3. That stage's vim commands used in real editing without fumbling.
4. A full workday without thinking about the moved keys.

## Plans

A plan is a TOML file listing every stage's 30 keys, with a name, blurb,
stats, vim commands and drill words for each. See
[examples/very-normal.toml](examples/very-normal.toml).

very-normal loads `~/.config/very-normal/plan.toml` (or `$VERY_NORMAL_PLAN`),
and falls back to Very Normal when there isn't one.

## Any keyboard

Oryx is only one way to get a stage onto a keyboard. `very-normal keymap
kanata` prints the current stage as a [kanata](https://github.com/jtroo/kanata)
config (macOS, Linux, Windows), and `very-normal keymap karabiner` prints a
[Karabiner-Elements](https://karabiner-elements.pqrs.org) complex modification
for `~/.config/karabiner/assets/complex_modifications/`. Add a stage number to
get a different one.

Both remap in software and expect the keyboard to send QWERTY, so a keyboard
whose firmware already has the stage must be left out, or its keys move twice.
The Karabiner rule only applies to the Mac's built-in keyboard (add
`--all-keyboards` to change that), so a remapped Voyager and a Very Normal
laptop can live side by side. The kanata config includes a commented
`macos-dev-names-include` line that does the same on macOS.

## Where things live

- Progress: `~/.local/share/very-normal/progress.json`, or `$VERY_NORMAL_DATA`.
  The format matches the web tutor's backup text.
- Oryx layout: the plan's `oryx_layout`, or `$VERY_NORMAL_LAYOUT`. Checking
  uses the GraphQL endpoint Oryx's own page uses, which is undocumented and
  needs a public layout link.
- URLs open with `open`, or `$VERY_NORMAL_OPEN` (`{url}` marks the address).

## Build

```sh
mise run test
mise run lint
cargo install --path .
```
