//! The coach: bossy about doing enough, and just as bossy about stopping.
//!
//! Short, frequent sessions teach fingers faster than long ones, and tired
//! sloppy reps teach sloppy habits. So: a daily goal, a nudge to wrap up a
//! long session, a forced break after a very long one, a daily cap, and a
//! warning when accuracy slides.

use chrono::{DateTime, Days, Local, NaiveDate, TimeZone};

use crate::store::Run;

/// Minutes a day to aim for.
pub const DAILY_GOAL: f64 = 20.0;
/// Minutes a day that count toward a streak.
pub const STREAK_MIN: f64 = 10.0;
/// Minutes a day after which the coach sends you away.
pub const DAILY_CAP: f64 = 45.0;
/// Minutes of continuous practice before a nudge to wrap up.
pub const SESSION_NUDGE: f64 = 15.0;
/// Minutes of continuous practice before a forced break.
pub const SESSION_LIMIT: f64 = 25.0;
/// A pause this long (minutes) ends a session, and is the break the coach asks for.
pub const BREAK: f64 = 5.0;
/// Old runs carry no duration; count each as this many seconds.
const UNKNOWN_SECS: f64 = 60.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Level {
    /// Doing well.
    Good,
    /// Keep going, or wrap up soon.
    Nudge,
    /// Stop: starting another drill needs an override.
    Stop,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Advice {
    pub level: Level,
    pub message: String,
    pub today_minutes: f64,
    pub session_minutes: f64,
    pub streak: u32,
}

impl Advice {
    pub fn blocks(&self) -> bool {
        self.level == Level::Stop
    }
}

fn secs(run: &Run) -> f64 {
    if run.secs > 0.0 {
        run.secs
    } else {
        UNKNOWN_SECS
    }
}

fn local(ms: u64) -> DateTime<Local> {
    Local
        .timestamp_millis_opt(ms as i64)
        .single()
        .unwrap_or_else(Local::now)
}

fn day(ms: u64) -> NaiveDate {
    local(ms).date_naive()
}

/// Today's local date as YYYY-MM-DD.
pub fn today(now_ms: u64) -> String {
    day(now_ms).to_string()
}

fn minutes_on(runs: &[Run], date: NaiveDate) -> f64 {
    // an empty f64 sum is -0.0, which prints as "-0"
    0.0 + runs
        .iter()
        .filter(|r| day(r.t) == date)
        .map(secs)
        .sum::<f64>()
        / 60.0
}

/// Days in a row with at least `STREAK_MIN` minutes, counting back from today
/// (or from yesterday, if today hasn't got there yet).
fn streak(runs: &[Run], today: NaiveDate) -> u32 {
    let mut date = if minutes_on(runs, today) >= STREAK_MIN {
        today
    } else {
        match today.checked_sub_days(Days::new(1)) {
            Some(d) => d,
            None => return 0,
        }
    };
    let mut n = 0;
    while minutes_on(runs, date) >= STREAK_MIN {
        n += 1;
        match date.checked_sub_days(Days::new(1)) {
            Some(d) => date = d,
            None => break,
        }
    }
    n
}

/// The runs in the current session: back from the latest run while the gaps
/// between them stay under the break length. Empty once you've had a break.
fn session(runs: &[Run], now_ms: u64) -> &[Run] {
    let gap_ms = (BREAK * 60_000.0) as u64;
    let Some(last) = runs.last() else {
        return &runs[..0];
    };
    if now_ms.saturating_sub(last.t) >= gap_ms {
        return &runs[..0];
    }
    let mut start = runs.len() - 1;
    while start > 0 {
        let began = runs[start]
            .t
            .saturating_sub((secs(&runs[start]) * 1000.0) as u64);
        if began.saturating_sub(runs[start - 1].t) >= gap_ms {
            break;
        }
        start -= 1;
    }
    &runs[start..]
}

fn plural(n: f64, word: &str) -> String {
    let n = n.round().max(0.0) as i64;
    if n == 1 {
        format!("1 {word}")
    } else {
        format!("{n} {word}s")
    }
}

pub fn assess(runs: &[Run], now_ms: u64) -> Advice {
    let today = day(now_ms);
    let today_minutes = minutes_on(runs, today);
    let streak = streak(runs, today);
    let current = session(runs, now_ms);
    let session_minutes = match (current.first(), current.last()) {
        (Some(first), Some(last)) => {
            let began = first.t.saturating_sub((secs(first) * 1000.0) as u64);
            last.t.saturating_sub(began) as f64 / 60_000.0
        }
        _ => 0.0,
    };
    let since_last = runs.last().map_or(f64::INFINITY, |r| {
        now_ms.saturating_sub(r.t) as f64 / 60_000.0
    });
    let advice = |level, message: String| Advice {
        level,
        message,
        today_minutes,
        session_minutes,
        streak,
    };

    if session_minutes >= SESSION_LIMIT {
        let wait = (BREAK - since_last).max(0.5);
        return advice(
            Level::Stop,
            format!(
                "Break time, babe. {} straight is plenty. Hands off for {}: stretch, water, look out a window.",
                plural(session_minutes, "minute"),
                plural(wait, "minute")
            ),
        );
    }
    if today_minutes >= DAILY_CAP {
        return advice(
            Level::Stop,
            format!(
                "{} today. Your fingers have clocked off; the rest happens while you sleep. Come back tomorrow.",
                plural(today_minutes, "minute")
            ),
        );
    }
    if current.len() >= 3 {
        let last3: Vec<f64> = current.iter().rev().take(3).rev().map(|r| r.acc).collect();
        if last3[0] > last3[1] && last3[1] > last3[2] && last3[2] < 92.0 {
            return advice(
                Level::Nudge,
                format!(
                    "Accuracy's sliding ({:.0}% → {:.0}%). Sloppy reps teach sloppy fingers. Take five.",
                    last3[0], last3[2]
                ),
            );
        }
    }
    if session_minutes >= SESSION_NUDGE {
        return advice(
            Level::Nudge,
            format!(
                "{} in. Finish this drill, then go stretch.",
                plural(session_minutes, "minute")
            ),
        );
    }
    if today_minutes >= DAILY_GOAL {
        return advice(
            Level::Good,
            format!(
                "{} today. Goal smashed. Anything more is a bonus, so keep it short.",
                plural(today_minutes, "minute")
            ),
        );
    }
    let left = DAILY_GOAL - today_minutes;
    let yesterday = today.checked_sub_days(Days::new(1));
    let skipped = yesterday.is_some_and(|y| minutes_on(runs, y) < STREAK_MIN) && !runs.is_empty();
    let message = if today_minutes < 0.5 && skipped {
        format!(
            "You skipped yesterday. Don't make it a habit: {DAILY_GOAL:.0} minutes today, in short bursts."
        )
    } else if today_minutes < 0.5 {
        format!("Nothing yet today. {DAILY_GOAL:.0} minutes, in two or three short bursts. Go.")
    } else {
        format!(
            "{} today, {} to go. Keep going, babe.",
            plural(today_minutes, "minute"),
            plural(left, "minute")
        )
    };
    let message = if streak >= 2 {
        format!("{message} {streak}-day streak, don't you dare break it.")
    } else {
        message
    };
    advice(Level::Nudge, message)
}

#[cfg(test)]
mod tests {
    use super::*;

    const MIN: u64 = 60_000;

    /// Noon today, local time, in epoch milliseconds.
    fn noon() -> u64 {
        Local::now()
            .date_naive()
            .and_hms_opt(12, 0, 0)
            .and_then(|t| Local.from_local_datetime(&t).single())
            .unwrap()
            .timestamp_millis() as u64
    }

    fn run(end_ms: u64, minutes: f64, acc: f64) -> Run {
        Run {
            t: end_ms,
            stage: 1,
            mode: "words".into(),
            wpm: 15.0,
            acc,
            secs: minutes * 60.0,
        }
    }

    /// Back-to-back runs of `each` minutes, the last ending at `end`.
    fn block(end: u64, count: u64, each: f64) -> Vec<Run> {
        let step = (each * 60_000.0) as u64;
        (0..count)
            .map(|i| run(end - (count - 1 - i) * step, each, 98.0))
            .collect()
    }

    #[test]
    fn nags_when_nothing_has_happened() {
        let a = assess(&[], noon());
        assert_eq!(a.level, Level::Nudge);
        assert!(a.message.contains("Nothing yet today"));
    }

    #[test]
    fn calls_out_a_skipped_day() {
        let two_days_ago = run(noon() - 48 * 60 * MIN, 15.0, 98.0);
        let a = assess(&[two_days_ago], noon());
        assert!(a.message.contains("skipped yesterday"), "{}", a.message);
    }

    #[test]
    fn counts_down_to_the_goal() {
        let runs = block(noon(), 4, 2.0);
        let a = assess(&runs, noon() + 10 * MIN);
        assert_eq!(a.level, Level::Nudge);
        assert!(
            a.message.contains("8 minutes today, 12 minutes to go"),
            "{}",
            a.message
        );
    }

    #[test]
    fn nudges_a_long_session_then_stops_it() {
        let runs = block(noon(), 8, 2.0);
        let a = assess(&runs, noon() + MIN);
        assert_eq!(a.level, Level::Nudge);
        assert!(a.message.contains("16 minutes in"), "{}", a.message);

        let runs = block(noon(), 13, 2.0);
        let a = assess(&runs, noon() + MIN);
        assert_eq!(a.level, Level::Stop);
        assert!(a.message.contains("Break time"));
        assert!(a.blocks());
    }

    #[test]
    fn a_break_resets_the_session() {
        let runs = block(noon(), 13, 2.0);
        let a = assess(&runs, noon() + 6 * MIN);
        assert_eq!(a.session_minutes, 0.0);
        assert_eq!(a.level, Level::Good, "{}", a.message);
    }

    #[test]
    fn caps_the_day() {
        let mut runs = block(noon() - 120 * MIN, 12, 2.0);
        runs.extend(block(noon(), 12, 2.0));
        let a = assess(&runs, noon() + 10 * MIN);
        assert_eq!(a.level, Level::Stop);
        assert!(a.message.contains("48 minutes today"), "{}", a.message);
    }

    #[test]
    fn notices_sliding_accuracy() {
        let mut runs = block(noon(), 3, 2.0);
        runs[0].acc = 97.0;
        runs[1].acc = 94.0;
        runs[2].acc = 89.0;
        let a = assess(&runs, noon() + MIN);
        assert!(a.message.contains("sliding"), "{}", a.message);
    }

    #[test]
    fn counts_a_streak() {
        let day = 24 * 60 * MIN;
        let mut runs = Vec::new();
        for back in (1..=3).rev() {
            runs.extend(block(noon() - back * day, 6, 2.0));
        }
        let a = assess(&runs, noon());
        assert_eq!(a.streak, 3);
        assert!(a.message.contains("3-day streak"), "{}", a.message);
    }
}
