# Brief

The coach's daily target is "20 minutes" and its voice is bossy ("Don't make it a habit", "Go.", "don't you dare break it", "You skipped yesterday" even after 9 minutes of practice). The tutor's theme is "there's no rush, take your time, it'll gradually come together". Make the daily target two 10-minute sessions, one in the morning and one later in the day, and rewrite the coach's messages in that calm voice. The 10-minute streak minimum and the 90% real-text gate stay as they are: anything under ~30 WPM isn't practical day to day, so the bar isn't lowered.

---

## Goal

The coach counts practice as sessions (two 10-minute sessions a day, at least an hour apart) and speaks in a calm, no-rush voice in `very-normal status`, the TUI coach line and the warm-up messages.

## Acceptance criteria

1. `coach::SESSION_MINUTES` is 10.0, `coach::SESSIONS_PER_DAY` is 2, and `coach::SESSION_GAP` is 60.0 (minutes); `coach::DAILY_GOAL` no longer exists.
2. `coach::sessions(runs, date)` groups the runs on that local date: a run starts a new group when it began (its `t` minus its duration) 60 or more minutes after the previous run's `t`. A group counts as a session when its summed run duration (using the existing 60-second default for runs without `secs`) is at least 10 minutes. It returns the number of sessions.
3. Ten 60-second runs with 30-second gaps between them make 1 session. Two such blocks three hours apart make 2. Two such blocks 30 minutes apart make 1. Nine 60-second runs make 0.
4. `Advice` has a `sessions_today: usize` field holding `sessions(runs, today)`.
5. With no runs today, the message contains "No rush" and mentions a 10-minute session, and does not contain "skipped", "Go." or "habit".
6. With no runs yesterday and none today (and older runs present), the message calls yesterday a rest day and doesn't contain "skipped" or "habit". With 9 minutes yesterday and none today, the message says nothing about yesterday.
7. With runs today but no completed session, the message states the minutes so far and that 10 makes a session.
8. With 1 session today and the last run less than 60 minutes ago, the message says to come back later today for the second. With the last run 60 or more minutes ago, it says the second session can start whenever you're ready.
9. With 2 or more sessions today, `level` is `Level::Good` and the message says the day's practice is done. It contains neither "smashed" nor "bonus".
10. A streak of 2 or more adds "{n} days in a row." to the message, and the message does not contain "dare".
11. The break, daily-cap and session-nudge messages keep their thresholds and levels but drop "babe" and imperatives like "Go". The break message still says how long to rest.
12. `very-normal status` prints the message, then `today {s}/2 sessions · {m} min · {n}-day streak · practising stage {k}`.
13. The TUI coach line shows `today {s}/2 sessions · session {m} min` in place of `today {x}/{y} min · session {m} min`.
14. Skipping the warm-up with `s` shows a message that contains no "Bold" and no warning about wrists.
15. The charts data's `goal` is `SESSION_MINUTES * SESSIONS_PER_DAY` (20).
16. README's "The coach" section describes two 10-minute sessions a day, at least an hour apart, in the calm voice, and no longer says the coach nags or "calls you out".
17. `cargo test` and `cargo clippy --all-targets -- -D warnings` pass.

## Out of scope

- The 90% real-text gate, the words-accuracy gate and the 10-minute streak minimum.
- Enforcing morning/afternoon clock times: only the 60-minute gap matters.
- The deterioration check's thresholds (only its wording may soften).
- The guided session flow (cards 02–04).

## Open questions

None.
