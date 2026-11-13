# Brief

After the warm-up, the guided session runs 10 minutes of practice chosen for you: one 60-second real-text run first while you're fresh, then new-keys and review drills. The screen shows only the text, a countdown and the key hint when you pause. If accuracy falls below 95% in two runs in a row, end the block early and say why.

---

## Goal

After the warm-up, the guided session runs a practice block of 10 minutes of drill time, picking the drills itself, on a screen that shows only what you're typing, then hands over to the dashboard.

## Acceptance criteria

1. `Phase` gains `Practice`. When the guided warm-up finishes, is skipped or is left with esc, the phase becomes `Phase::Practice` (not `Dashboard`).
2. The block's drills come from `practice_mode(i)` for the i-th saved run of the block (0-based): `Mode::Real` for 0, then `Mode::Words` for odd i and `Mode::Review` for even i ≥ 2.
3. In `Phase::Practice`, enter starts the next drill when not typing. Each finished drill is saved as a run exactly as in the dashboard, so the gate and coach count it.
4. The block tracks its drill time as the summed `secs` of the runs saved during it. Once that reaches 600 seconds after a run is saved, the block ends. A drill in progress is never cut short by the block.
5. If the last two runs saved in the block both have accuracy below 95.0, the block ends early, and the message names the two accuracies and says to stop for now in the calm, no-rush voice (no "Go", no blame).
6. When the block ends, the phase becomes `Phase::Dashboard` and the block's end message is shown there.
7. Esc while typing abandons the drill without saving, as in the dashboard, and stays in `Phase::Practice`. Esc while not typing ends the block and moves to `Phase::Dashboard`.
8. The coach's Stop advice (break or daily cap) doesn't block drills inside a practice block.
9. Rendered to an 80×24 `TestBackend` in `Phase::Practice`, the screen contains the drill text, a line `{m}:{ss} of practice left` (600 minus the block's drill time, floored at 0:00) and the current drill's label, plus a help line. It contains none of: the coach message, "Gate"/"gate", the drill picker, the stage bar, "oryx".
10. In `Phase::Practice`, the keyboard map is rendered only while the drill's hint is showing (after `HINT_AFTER` without progress), with the wanted key lit as in the dashboard.
11. After each saved run in the block, the screen shows that run's WPM and accuracy and "press enter for the next drill" until enter is pressed.
12. `cargo test` and `cargo clippy --all-targets -- -D warnings` pass.

## Out of scope

- The end-of-session report (card 04). Until it exists, the block hands over to the dashboard.
- Changing how drills are built, timed or scored.
- Recall, links and mirror drills in the block.
- Pausing and resuming the block across app restarts.

## Open questions

None.
