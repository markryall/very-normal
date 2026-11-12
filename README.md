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

## Install

```sh
brew install markryall/tap/very-normal
# or
cargo install very-normal
```

## Make your own layout

very-normal can build a layout from your own typing, and a plan of stages
that gets you there from QWERTY. The first time you run `very-normal`, it
offers to do this from the messages you've typed to Claude Code. It only
reads your messages: never Claude's replies, tool output or pastes. Any time
after that:

```sh
very-normal make-plan --name "My Layout"
```

Anything else you've written makes the layout fit you better. `--text` adds
files, folders of .md, .txt and .org files, or `-` for stdin. `--no-claude`
leaves your Claude Code messages out:

```sh
very-normal make-plan --name "My Layout" --text ~/notes ~/Documents/emails.txt
```

- `--lock` takes keys that stay where QWERTY has them. The default,
  `qwzxcv,./`, keeps Cmd+Q, W, Z, X, C and V. Each locked key costs the
  layout a little, and locking nothing is fine too.
- `--oryx-layout <id>` reads your home row holds from Oryx, if you have a ZSA
  board.
- `very-normal make-plan --help` lists the rest.

It takes a few seconds, and nothing leaves your machine. It writes two files
to `~/.config/very-normal/plans/`, named after your layout:

- `<name>.toml`: your layout and its stages, with drill words from your own
  writing. Keep it private, and look through the words for names you'd rather
  not see on screen.
- `<name>.md`: your layout and how it scores against QWERTY, Colemak-DH,
  Norman and Very Normal on your typing. It has no words or text in it.

Making a plan never touches the plan you're practising or your progress, so
rerun it as often as you like with other text, locks or names. Your first
plan becomes the active one. After that, switch with `very-normal use
<name>` (`very-normal use very-normal` goes back to the built-in plan). Each
plan keeps its own progress, so you can switch back and pick up where you
left off. A plan whose stages are exactly the same as the one you're
practising takes your progress with it. `very-normal plan` lists the plans
you've made, and `very-normal remove <name>` deletes one.

**Share your layout.** I'm curious how different everyone's layouts turn
out. `very-normal share` opens a GitHub issue with the summary of the plan
you made most recently filled in (`very-normal share <name>` for another).
You can read it, and change it, before you press Submit. The summary also
says how much the search agreed with itself. It runs 20 times from random
starting points, and many layouts score almost the same, so some of the
difference between two people's layouts is chance. The summary shows how
much.

See [MAKE_YOUR_OWN.md](MAKE_YOUR_OWN.md) for how the layout is scored and
searched for.

## Use

```sh
very-normal                  # the trainer
very-normal status           # what the coach thinks of your practice today
very-normal next             # the Oryx edits for the next stage, and the link to make them
very-normal check            # read the layout from Oryx and compare it with your stage
very-normal plan             # which plan you're on, and its stages
very-normal make-plan        # make a layout and plan from your own typing
very-normal use <name>       # switch plans; each keeps its own progress
very-normal remove <name>    # delete a plan you made, with its summary and progress
very-normal share [name]     # share a plan's summary (no text) on GitHub
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
  The format matches the web tutor's backup text. Progress on the plans
  you're not practising waits in `plans/` next to it.
- Plans you've made: `~/.config/very-normal/plans/`, next to the active
  `plan.toml`.
- Oryx layout: the plan's `oryx_layout`, or `$VERY_NORMAL_LAYOUT`. Checking
  uses the GraphQL endpoint Oryx's own page uses, which is undocumented and
  needs a public layout link.
- URLs open with `open` (`xdg-open` on Linux), or `$VERY_NORMAL_OPEN` (`{url}`
  marks the address).

## Build

```sh
mise run test
mise run lint
cargo install --path .
```
