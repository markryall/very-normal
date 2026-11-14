# Changelog

## Unreleased

- Plain `very-normal` starts a guided session. It shows the layout on its own, then the warm-up, then ten minutes of practice it picks for you, real text first. It stops early after two sloppy runs. The old screen moves to `very-normal dashboard`.
- The guided session ends with a report: this session against recent ones, your slowest keys, and an unhurried look at the next stage.
- The guided session suggests a quiet place and headphones, and `m` plays an ambient stream in the background.
- The coach counts two calm 10-minute sessions a day instead of nagging toward 20 minutes.
- The practice screen shows the key map the whole time and nothing on it changes when you pause, so the text never moves while you type.
- Keys you type while the music plays reach the drill. mpv used to read the terminal itself and swallow some of them.
- Exits quietly when a pipe closes early instead of panicking on a broken pipe.

## 0.3.0 - 2026-11-13

- Start from the layout you type on now: `make-plan --from` takes QWERTY, Dvorak, Colemak, Colemak-DH, Workman, Norman or your own 30 keys, and the first run asks which one you use.
- Locks keep keys where they are on your current layout, summaries compare against all the known layouts, and keymaps send the quote key to whatever the 30 keys leave out.

## 0.2.1 - 2026-11-13

- Prebuilt binaries for macOS and Linux, installable from the Homebrew tap or with a shell installer.

## 0.2.0 - 2026-11-13

- `make-plan` builds your own layout and plan inside the binary, with no Python, into `plans/` without touching the plan you're practising.
- The first run offers to make a plan from your Claude Code messages and shows how to add other writing.
- `use` switches plans and keeps each plan's progress. It carries your progress over to a plan whose stages are exactly the same.
- `remove` deletes a plan you made, along with its summary and progress.
- `share` opens a GitHub issue with your layout, its stages from QWERTY, and how much the search's restarts agreed.
- Links open with `xdg-open` on Linux.
- Words drills lean on your trickiest keys. Words that use them come up as much as 3x as often, and the drill says which keys are getting extra reps.
- Charts: a self-contained page of speed, accuracy, per-stage climbs, key heatmaps, typos, slow pairs and practice habits.
- A kinder coach: it wants 97% in 3 of the last 5 runs, rounds accuracy the same way for showing and judging, offers encouragement and speed records instead of resets, and suggests a break when a session goes backwards.
- Karabiner keymaps only touch the built-in keyboard unless you pass `--all-keyboards`, and the kanata config explains device filtering.

## 0.1.0 - 2026-11-12

- A terminal tutor for moving onto the Very Normal layout on a Voyager in stages, with a bossy practice coach.
- Stages, words and Oryx settings load from a plan file, with Very Normal as the built-in example.
- A guided one-minute warm-up before the first drill of the day.
- `very-normal keymap` turns any stage into a kanata config or a Karabiner rule, for keyboards without Oryx.
- The header and coach stay visible in short terminals.
