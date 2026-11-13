# Brief

The guided session ends with a report: this session against the last few (words and real-text WPM and accuracy, slowest keys), then a feasible goal for the next layout switch: which gate items are left and, from the recent real-text trend, roughly how many more sessions until the real-text bar is reached. Theme: there's no rush, it'll gradually come together. The 90% bar stays.

---

## Goal

When a practice block ends, the guided session shows a report comparing this session with recent ones and estimating, without pressure, how far off the next layout switch is.

## Acceptance criteria

1. `Phase` gains `Report`. When a practice block ends (by time or early), the phase becomes `Phase::Report`, not `Dashboard`.
2. `report::session(progress, stage, block_start_ms) -> Vec<String>` is a pure function that produces the report lines. Runs with `t >= block_start_ms` are "this session". "Earlier sessions" are groups of earlier runs at the same stage, grouped with the coach's 60-minute gap rule (card 01), counting only groups that contain a run of that mode.
3. For each of `real`, `words` and `review` that has a run this session, there's a line with the mode label, the number of runs, and mean WPM and accuracy to one decimal. If there are earlier sessions with that mode, the line adds the change against the mean over the last 3 of them, signed (e.g. `+1.2 WPM, -0.4%`). Otherwise it says `first session`.
4. A line names this session's 3 slowest keys by mean ms per press. It uses letters with at least 3 presses this session, excludes space, and gives each key's ms this session and, where earlier sessions have data, its mean ms over those sessions.
5. A line lists the labels of the gate items for this stage that are not `ok`, or says every gate item is done.
6. Real-text trend: take each session's best real run at 95%+ accuracy, for this session and up to 4 earlier ones. With a baseline and fewer than 3 such sessions, a line says there aren't enough sessions yet to see a trend. With 3 or more and a least-squares slope above 0, a line says "at this pace, roughly {n} more sessions to reach {target} WPM", where n = ceil((target − last fitted value) / slope), at least 1. With a slope of 0 or less, a line says there's no upward trend yet and that this is normal. If the real-text gate item is already ok, the line says real text is there instead.
7. Any line about time or trend contains none of "only", "behind", "hurry", "must" or "should have".
8. Unit tests cover items 3–6 with constructed `Progress` values, including: no earlier sessions; a rising real-text trend (16, 17, 18 WPM over 3 sessions with a 30 WPM baseline and target 27 → "roughly 9 more sessions"); and a flat trend.
9. Rendered to an 80×24 `TestBackend` in `Phase::Report`, the screen shows the report lines and a help line with `enter` and `q`. It contains no drill text and no drill picker.
10. In `Phase::Report`, enter moves to `Phase::Dashboard`, and `q` and Ctrl-C quit.
11. README describes the guided session's four steps: layout, warm-up, 10 minutes of practice, report.
12. `cargo test` and `cargo clippy --all-targets -- -D warnings` pass.

## Out of scope

- Changing the gate's thresholds.
- Saving reports, or showing them outside the TUI (`status`, charts).
- Projecting the words-accuracy or manual gate items.

## Open questions

None.
