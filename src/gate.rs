//! The mastery gate: what has to be true before climbing a stage.

use crate::layout;
use crate::store::Progress;

/// Real text has to get back to this share of your baseline speed.
pub const SPEED_SHARE: f64 = 0.9;
pub const WORDS_ACCURACY: f64 = 97.0;
/// How many of the last `WORDS_WINDOW` new-keys runs must reach `WORDS_ACCURACY`.
pub const WORDS_NEEDED: usize = 3;
pub const WORDS_WINDOW: usize = 5;
pub const REAL_ACCURACY: f64 = 95.0;

/// The manual boxes, and the key that ticks each one in the TUI.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tick {
    Kept,
    Vim,
    Workday,
}

impl Tick {
    pub fn key(self) -> char {
        match self {
            Tick::Kept => 'k',
            Tick::Vim => 'v',
            Tick::Workday => 'w',
        }
    }

    pub fn toggle(self, progress: &mut Progress, stage: usize) {
        let m = progress.manual_mut(stage);
        match self {
            Tick::Kept => m.oryx = !m.oryx,
            Tick::Vim => m.vim = !m.vim,
            Tick::Workday => m.workday = !m.workday,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Item {
    pub label: String,
    pub ok: bool,
    pub detail: String,
    pub tick: Option<Tick>,
}

pub fn items(progress: &Progress, stage: usize) -> Vec<Item> {
    let manual = progress.manual(stage);
    if stage == 0 {
        let n = progress.runs_for(0, "real").len();
        let detail = match (n, progress.baseline()) {
            (0, Some(b)) => format!("Typed in: {b:.0} WPM"),
            (0, None) => "Do three real-text runs, or run `very-normal baseline <wpm>`".into(),
            (n, Some(b)) => format!("{n} run(s), averaging {b:.0} WPM"),
            (n, None) => format!("{n} run(s)"),
        };
        return vec![
            Item {
                label: "Planck setup kept safe (old revision or firmware file)".into(),
                ok: manual.oryx,
                detail: String::new(),
                tick: Some(Tick::Kept),
            },
            Item {
                label: "Baseline recorded".into(),
                ok: n >= 3 || manual.wpm > 0.0,
                detail,
                tick: None,
            },
        ];
    }

    let words = progress.runs_for(stage, "words");
    let recent: Vec<_> = words.iter().rev().take(WORDS_WINDOW).collect();
    let hits = words_hits(progress, stage);
    let words_ok = hits >= WORDS_NEEDED;
    let words_detail = if recent.is_empty() {
        "No new-keys runs yet".into()
    } else {
        format!(
            "{} of {WORDS_NEEDED} so far. Recent: {}",
            hits.min(WORDS_NEEDED),
            recent
                .iter()
                .rev()
                .map(|r| format!("{:.1}%", r.acc))
                .collect::<Vec<_>>()
                .join(", ")
        )
    };

    let baseline = progress.baseline();
    let real = progress.runs_for(stage, "real");
    let best = real
        .iter()
        .rev()
        .take(5)
        .filter(|r| r.acc >= REAL_ACCURACY)
        .max_by(|a, b| a.wpm.total_cmp(&b.wpm));
    let (real_ok, real_detail) = match baseline {
        None => (false, "Record a baseline in stage 0 first".to_string()),
        Some(b) => {
            let target = b * SPEED_SHARE;
            let ok = best.is_some_and(|r| r.wpm >= target);
            let best = best.map_or("no qualifying run yet".to_string(), |r| {
                format!("best recent {:.0} WPM at {:.0}%", r.wpm, r.acc)
            });
            (ok, format!("Target {target:.0} WPM; {best}"))
        }
    };

    vec![
        Item {
            label: format!(
                "New keys at {WORDS_ACCURACY:.0}%+ in {WORDS_NEEDED} of your last {WORDS_WINDOW} runs"
            ),
            ok: words_ok,
            detail: words_detail,
            tick: None,
        },
        Item {
            label: format!(
                "Real text at {:.0}% of baseline, {REAL_ACCURACY:.0}%+ accuracy",
                SPEED_SHARE * 100.0
            ),
            ok: real_ok,
            detail: real_detail,
            tick: None,
        },
        Item {
            label: format!(
                "Used {} in real editing without fumbling",
                layout::vim(stage).join(" ")
            ),
            ok: manual.vim,
            detail: String::new(),
            tick: Some(Tick::Vim),
        },
        Item {
            label: "A full workday without thinking about the moved keys".into(),
            ok: manual.workday,
            detail: String::new(),
            tick: Some(Tick::Workday),
        },
    ]
}

/// How many of the last few new-keys runs reached the accuracy bar.
pub fn words_hits(progress: &Progress, stage: usize) -> usize {
    progress
        .runs_for(stage, "words")
        .iter()
        .rev()
        .take(WORDS_WINDOW)
        .filter(|r| r.acc >= WORDS_ACCURACY)
        .count()
}

pub fn passed(progress: &Progress, stage: usize) -> bool {
    items(progress, stage).iter().all(|i| i.ok)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::Run;

    fn run(stage: usize, mode: &str, wpm: f64, acc: f64) -> Run {
        Run {
            t: 0,
            stage,
            mode: mode.into(),
            wpm,
            acc,
            secs: 60.0,
            ..Run::default()
        }
    }

    #[test]
    fn stage_zero_passes_with_a_typed_baseline_and_the_box() {
        let mut p = Progress::default();
        assert!(!passed(&p, 0));
        p.manual_mut(0).wpm = 35.0;
        Tick::Kept.toggle(&mut p, 0);
        assert!(passed(&p, 0));
    }

    #[test]
    fn stage_one_needs_three_accurate_runs_speed_and_both_boxes() {
        let mut p = Progress::default();
        p.manual_mut(0).wpm = 30.0;
        for acc in [98.0, 99.0, 97.5] {
            p.runs.push(run(1, "words", 20.0, acc));
        }
        p.runs.push(run(1, "real", 28.0, 96.0));
        assert!(!passed(&p, 1));
        Tick::Vim.toggle(&mut p, 1);
        Tick::Workday.toggle(&mut p, 1);
        assert!(passed(&p, 1));

        p.runs.push(run(1, "words", 25.0, 90.0));
        p.runs.push(run(1, "words", 25.0, 91.0));
        assert!(passed(&p, 1), "a couple of wobbly runs don't wipe progress");
        p.runs.push(run(1, "words", 25.0, 92.0));
        assert!(!passed(&p, 1), "but the good runs have to be recent");
    }

    #[test]
    fn slow_or_sloppy_real_text_does_not_count() {
        let mut p = Progress::default();
        p.manual_mut(0).wpm = 30.0;
        p.runs.push(run(1, "real", 26.0, 99.0));
        p.runs.push(run(1, "real", 40.0, 90.0));
        assert!(!items(&p, 1)[1].ok);
    }
}
