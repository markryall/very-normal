# very-normal

A terminal typing tutor for moving a ZSA Voyager from QWERTY to the **Very
Normal** layout a few keys at a time. Very Normal is a layout optimised on
318k characters of my own typing, with Q, W, Z, X, C and V locked so every Mac
shortcut stays where it was.

```
q w y d g   b h a u j
i l s t m   p n o e r  '
z x c v k   ; f , . /
```

There are nine stages. Each one moves three to five keys, and you only change
a key's tap in Oryx, so home row mods stay on the same fingers throughout.

## Use

```sh
very-normal                  # the trainer
very-normal next             # the Oryx edits for the next stage, and the link to make them
very-normal check            # read the layout from Oryx and compare it with your stage
very-normal stage 2          # set the stage you're practising
very-normal baseline 35      # type in a baseline instead of recording one
very-normal import backup.json   # merge a backup copied out of the web tutor
very-normal export           # print progress as JSON
```

In the trainer:

- `←` `→` change stage. `1`–`6` pick a drill: key recall, new keys, review,
  60 seconds of real text, markdown links, or one-handed mirror practice.
- `enter` starts typing. Wrong keys don't advance. If you pause, the key you
  want lights up on the map. `esc` stops without saving.
- `c` checks Oryx and `o` opens the layout there.
- `k`, `v` and `w` tick the gate's manual boxes.

## The gate

Before climbing a stage:

1. Three new-keys runs in a row at 97%+ accuracy.
2. Real text back to 90% of your baseline speed at 95%+ accuracy.
3. That stage's vim commands used in real editing without fumbling.
4. A full workday without thinking about the moved keys.

## Where things live

- Progress: `~/.local/share/very-normal/progress.json`, or `$VERY_NORMAL_DATA`.
  The format matches the web tutor's backup text.
- Oryx layout: `Xx6pM`, or `$VERY_NORMAL_LAYOUT`. Checking uses the GraphQL
  endpoint Oryx's own page uses, which is undocumented and needs a public
  layout link.
- URLs open with `open`, or `$VERY_NORMAL_OPEN` (`{url}` marks the address).

## Build

```sh
mise run test
mise run lint
cargo install --path .
```
