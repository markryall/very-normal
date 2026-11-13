# Brief

The trainer shows too much at once (header, stage bar, coach, keyboard, Oryx edits, gate, drill picker, runs, long help line), so it's hard to know what you're meant to be doing. Plain `very-normal` should start a guided session instead: first the current layout full screen with this stage's new keys highlighted (enter moves on), then the breathing and stretching warm-up, every session. The existing dashboard stays reachable from a key and from `very-normal dashboard`.

---

## Goal

Running `very-normal` with no arguments starts a guided session that shows the layout on its own screen, then the warm-up on its own screen, then lands on the existing dashboard. `very-normal dashboard` opens the dashboard directly.

## Acceptance criteria

1. `App` has a `phase` of type `Phase` with at least the variants `Layout`, `Warmup` and `Dashboard`. `App::new` takes the starting phase.
2. `very-normal` with no arguments starts in `Phase::Layout`. `very-normal dashboard` starts in `Phase::Dashboard`, and the dashboard behaves as the trainer does today. `very-normal --help` lists `dashboard`.
3. Rendered to an 80×24 `TestBackend` in `Phase::Layout`, the screen contains the plan name, `stage {n}` and the stage name, the keyboard map, a line naming this stage's new keys (each `layout::changes(stage)` entry's target key), and a help line with `enter`, `d` and `q`. It contains none of: the coach message, "Gate"/"gate", the drill picker labels, "oryx".
4. On the layout screen, the keys new in this stage are styled differently from the other keys on the map.
5. In `Phase::Layout`, enter moves to `Phase::Warmup` with a fresh `Warmup` started, whether or not `progress.warmed_up` is today. `d` moves to `Phase::Dashboard`, and `q` quits.
6. Rendered in `Phase::Warmup`, the screen shows the warm-up steps and a help line, and none of: the keyboard map, the coach message, "Gate"/"gate", the drill picker labels.
7. In `Phase::Warmup`, the existing keys work as today (enter or space skips a step, `s` skips the warm-up). When the warm-up finishes or is skipped, `progress.warmed_up` is set to today and the phase becomes `Phase::Dashboard`. Esc also moves to `Phase::Dashboard` without setting `warmed_up`.
8. Entering the dashboard after a guided warm-up does not start a second warm-up when enter is pressed for a drill.
9. Ctrl-C quits in every phase.
10. README's "Use" section says plain `very-normal` starts a guided session and lists `very-normal dashboard`.
11. `cargo test` and `cargo clippy --all-targets -- -D warnings` pass.

## Out of scope

- The practice block and the report (cards 03 and 04). Until they exist, the warm-up hands over to the dashboard.
- Changing the dashboard's own layout or keys, beyond what is needed for it to be one phase among several.
- Auto-advancing the layout screen on a timer.
- The first-run welcome flow, which still runs before anything else.

## Open questions

None.
