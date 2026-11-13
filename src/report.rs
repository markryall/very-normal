//! Plain-text explanations shared by the command line and the TUI.

use std::collections::BTreeMap;

use crate::drill::Mode;
use crate::oryx::Snapshot;
use crate::store::{LetterStat, Progress, Run};
use crate::{coach, gate, layout};

/// The Oryx edits that turn `stage - 1` into `stage`.
pub fn edits(stage: usize) -> Vec<String> {
    let mut out = Vec::new();
    for c in layout::changes(stage) {
        let hold =
            layout::modifier(c.index).map_or(String::new(), |m| format!("  (keep the {m} hold)"));
        out.push(format!("key that types {} → {}{hold}", c.from, c.to));
    }
    for c in layout::changes(stage).iter().filter(|c| c.right_hand()) {
        out.push(format!(
            "mirror layer: key that types {} → {}",
            c.from, c.to
        ));
    }
    out
}

pub fn next_stage(current: usize, url: &str) -> Vec<String> {
    if current >= layout::last() {
        return vec!["You're on the last stage. Nothing left to edit.".into()];
    }
    let next = current + 1;
    let mut out = vec![
        format!("Stage {next}: {}", layout::name(next)),
        format!("Edit: {url}"),
    ];
    out.extend(edits(next).into_iter().map(|e| format!("  {e}")));
    out.push(format!(
        "  vim to practise: {}",
        layout::vim(next).join(" ")
    ));
    out
}

/// The same, squeezed into a few lines for the TUI.
pub fn next_stage_compact(current: usize, url: &str) -> Vec<String> {
    if current >= layout::last() {
        return vec!["You're on the last stage. Nothing left to edit.".into()];
    }
    let next = current + 1;
    let changes = layout::changes(next);
    let swap = |c: &layout::Change| format!("{}→{}", c.from, c.to);
    let keys: Vec<String> = changes
        .iter()
        .map(|c| match layout::modifier(c.index) {
            Some(m) => format!("{} (keep {m})", swap(c)),
            None => swap(c),
        })
        .collect();
    let mirror: Vec<String> = changes
        .iter()
        .filter(|c| c.right_hand())
        .map(swap)
        .collect();
    let mut out = vec![
        format!("Stage {next}: {}", layout::name(next)),
        format!("keys: {}", keys.join("  ")),
    ];
    if !mirror.is_empty() {
        out.push(format!("mirror: {}", mirror.join("  ")));
    }
    out.push(format!("vim: {}", layout::vim(next).join(" ")));
    out.push(url.to_string());
    out
}

/// What Oryx says the board is on, measured against the stage you practise.
pub fn check(snap: &Snapshot, current: usize) -> Vec<String> {
    let mut out = vec![format!(
        "{} · revision {} · {}",
        snap.title, snap.revision, snap.created
    )];
    let findings = snap.compare(current);
    match snap.stage() {
        Some(s) if s == current && findings.is_empty() => {
            out.push(format!(
                "Matches stage {current}: every key, hold and mirror key."
            ));
        }
        Some(s) if s == current + 1 && snap.compare(s).is_empty() => {
            out.push(format!(
                "Stage {s}'s edits are all in. Flash it and climb to stage {s}."
            ));
        }
        Some(s) if snap.compare(s).is_empty() => {
            out.push(format!(
                "Matches stage {s}, but you're practising stage {current}."
            ));
        }
        _ => {
            let target =
                if current < layout::last() && snap.compare(current + 1).len() < findings.len() {
                    current + 1
                } else {
                    current
                };
            let f = snap.compare(target);
            out.push(format!(
                "Closest to stage {target}, with {} difference(s):",
                f.len()
            ));
            out.extend(f.iter().take(8).map(|f| format!("  {f}")));
            if f.len() > 8 {
                out.push(format!("  and {} more", f.len() - 8));
            }
        }
    }
    out
}

/// The end of a guided session: how it went against the last few, and how
/// far off the next stage is. Runs from `block_start_ms` on are this session.
pub fn session(progress: &Progress, stage: usize, block_start_ms: u64) -> Vec<String> {
    let at_stage: Vec<&Run> = progress.runs.iter().filter(|r| r.stage == stage).collect();
    let (earlier, now): (Vec<&Run>, Vec<&Run>) =
        at_stage.iter().partition(|r| r.t < block_start_ms);
    let earlier = coach::group(&earlier);
    let mut out = Vec::new();

    for mode in [Mode::Real, Mode::Words, Mode::Review] {
        let this: Vec<&Run> = now
            .iter()
            .copied()
            .filter(|r| r.mode == mode.id())
            .collect();
        if this.is_empty() {
            continue;
        }
        let (wpm, acc) = means(&this);
        let before: Vec<&Run> = earlier
            .iter()
            .filter(|g| g.iter().any(|r| r.mode == mode.id()))
            .rev()
            .take(3)
            .flat_map(|g| g.iter().copied().filter(|r| r.mode == mode.id()))
            .collect();
        let compared = if before.is_empty() {
            "first session".to_string()
        } else {
            let (w, a) = means(&before);
            format!(
                "{:+.1} WPM, {:+.1}% against recent sessions",
                wpm - w,
                acc - a
            )
        };
        out.push(format!(
            "{}: {} run{}, {wpm:.1} WPM at {acc:.1}% ({compared})",
            mode.label(),
            this.len(),
            if this.len() == 1 { "" } else { "s" }
        ));
    }

    let mine = letters(&now);
    let before = letters(&earlier.iter().flatten().copied().collect::<Vec<_>>());
    let mut slow: Vec<(&String, f64)> = mine
        .iter()
        .filter(|(c, l)| c.as_str() != " " && l.n >= 3)
        .map(|(c, l)| (c, l.ms / l.n as f64))
        .collect();
    slow.sort_by(|a, b| b.1.total_cmp(&a.1));
    if !slow.is_empty() {
        let keys: Vec<String> = slow
            .iter()
            .take(3)
            .map(|(c, ms)| match before.get(*c).filter(|l| l.n > 0) {
                Some(l) => format!("{c} {ms:.0}ms (earlier {:.0}ms)", l.ms / l.n as f64),
                None => format!("{c} {ms:.0}ms"),
            })
            .collect();
        out.push(format!("Slowest keys: {}", keys.join(" · ")));
    }

    let items = gate::items(progress, stage);
    let left: Vec<&str> = items
        .iter()
        .filter(|i| !i.ok)
        .map(|i| i.label.as_str())
        .collect();
    out.push(if left.is_empty() {
        "Every gate item for this stage is done.".to_string()
    } else {
        format!("Still to come before the next stage: {}.", left.join("; "))
    });

    let real_done = stage > 0
        && items
            .iter()
            .any(|i| i.label.starts_with("Real text") && i.ok);
    if real_done {
        out.push("Real text is there for this stage.".into());
    } else if stage > 0 {
        out.push(match progress.baseline() {
            None => "Record a baseline in stage 0 to see a real-text trend.".into(),
            Some(b) => trend(&earlier, &now, b * gate::SPEED_SHARE),
        });
    }
    out
}

fn means(runs: &[&Run]) -> (f64, f64) {
    let n = runs.len() as f64;
    (
        runs.iter().map(|r| r.wpm).sum::<f64>() / n,
        runs.iter().map(|r| r.acc).sum::<f64>() / n,
    )
}

fn letters(runs: &[&Run]) -> BTreeMap<String, LetterStat> {
    let mut out: BTreeMap<String, LetterStat> = BTreeMap::new();
    for r in runs {
        for (c, l) in &r.letters {
            out.entry(c.clone()).or_default().add(l);
        }
    }
    out
}

/// Each session's best accurate real-text run, and where that line is heading.
fn trend(earlier: &[Vec<&Run>], now: &[&Run], target: f64) -> String {
    let best = |runs: &[&Run]| {
        runs.iter()
            .filter(|r| r.mode == "real" && r.acc >= gate::REAL_ACCURACY)
            .map(|r| r.wpm)
            .reduce(f64::max)
    };
    let mut points: Vec<f64> = earlier.iter().filter_map(|g| best(g)).collect();
    let keep = if best(now).is_some() { 4 } else { 5 };
    points = points.split_off(points.len().saturating_sub(keep));
    points.extend(best(now));
    if points.len() < 3 {
        return "Not enough sessions yet to see a real-text trend. Give it a few.".into();
    }
    let n = points.len() as f64;
    let mean_x = (n - 1.0) / 2.0;
    let mean_y = points.iter().sum::<f64>() / n;
    let (mut sxy, mut sxx) = (0.0, 0.0);
    for (i, y) in points.iter().enumerate() {
        let dx = i as f64 - mean_x;
        sxy += dx * (y - mean_y);
        sxx += dx * dx;
    }
    let slope = sxy / sxx;
    if slope <= 0.0 {
        return "No upward real-text trend yet. That's normal; it'll gradually come together."
            .into();
    }
    let fitted = mean_y + slope * (n - 1.0 - mean_x);
    let sessions = ((target - fitted) / slope).ceil().max(1.0) as u64;
    format!(
        "At this pace, roughly {sessions} more session{} to reach {target:.0} WPM in real text. It'll gradually come together.",
        if sessions == 1 { "" } else { "s" }
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stage_two_edits_keep_holds_and_list_mirror_keys() {
        let e = edits(2);
        assert!(
            e.contains(&"key that types k → o  (keep the alt hold)".to_string()),
            "{e:?}"
        );
        assert_eq!(e.iter().filter(|l| l.starts_with("mirror")).count(), 3);
    }

    #[test]
    fn nothing_is_left_after_the_last_stage() {
        assert_eq!(next_stage(layout::last(), "u").len(), 1);
        assert_eq!(next_stage(0, "u")[1], "Edit: u");
    }

    fn run(t: u64, mode: &str, wpm: f64, acc: f64) -> Run {
        Run {
            t,
            stage: 1,
            mode: mode.into(),
            wpm,
            acc,
            secs: 60.0,
            ..Run::default()
        }
    }

    const HOUR: u64 = 3_600_000;
    const START: u64 = 1_000 * HOUR;

    fn progress(runs: Vec<Run>) -> Progress {
        let mut p = Progress {
            stage: 1,
            runs,
            ..Progress::default()
        };
        p.manual_mut(0).wpm = 30.0;
        p
    }

    fn calm(lines: &[String]) {
        for l in lines {
            for w in ["only", "behind", "hurry", "must", "should have"] {
                assert!(!l.contains(w), "{w:?} in {l}");
            }
        }
    }

    #[test]
    fn a_first_session_has_nothing_to_compare() {
        let p = progress(vec![
            run(START + 60_000, "real", 12.0, 96.0),
            run(START + 120_000, "words", 13.0, 98.0),
        ]);
        let r = session(&p, 1, START);
        assert!(
            r[0].starts_with("real text 60s: 1 run, 12.0 WPM at 96.0% (first session)"),
            "{r:?}"
        );
        assert!(
            r[1].starts_with("new keys: 1 run, 13.0 WPM at 98.0% (first session)"),
            "{r:?}"
        );
        assert!(
            r.iter().any(|l| l.contains("Not enough sessions yet")),
            "{r:?}"
        );
        assert!(r.iter().any(|l| l.starts_with("Still to come")), "{r:?}");
        calm(&r);
    }

    #[test]
    fn compares_with_recent_sessions() {
        let p = progress(vec![
            run(START - 5 * HOUR, "words", 10.0, 97.0),
            run(START - 3 * HOUR, "words", 12.0, 99.0),
            run(START + 60_000, "words", 13.0, 98.5),
        ]);
        let r = session(&p, 1, START);
        assert!(
            r[0].contains("(+2.0 WPM, +0.5% against recent sessions)"),
            "{r:?}"
        );
    }

    #[test]
    fn names_the_slowest_keys() {
        let mut now = run(START + 60_000, "words", 13.0, 98.0);
        for (c, ms) in [
            ("e", 4_000.0),
            ("l", 3_000.0),
            ("t", 1_500.0),
            ("a", 9_000.0),
            (" ", 9_000.0),
        ] {
            let n = if c == "a" { 2 } else { 4 };
            now.letters.insert(c.into(), LetterStat { n, miss: 0, ms });
        }
        let mut before = run(START - 3 * HOUR, "words", 12.0, 98.0);
        before.letters.insert(
            "e".into(),
            LetterStat {
                n: 4,
                miss: 0,
                ms: 6_000.0,
            },
        );
        let r = session(&progress(vec![before, now]), 1, START);
        let line = r.iter().find(|l| l.starts_with("Slowest keys")).unwrap();
        assert_eq!(
            line,
            "Slowest keys: e 1000ms (earlier 1500ms) · l 750ms · t 375ms"
        );
    }

    #[test]
    fn projects_a_rising_real_text_trend() {
        let p = progress(vec![
            run(START - 6 * HOUR, "real", 16.0, 96.0),
            run(START - 3 * HOUR, "real", 17.0, 97.0),
            run(START + 60_000, "real", 18.0, 96.0),
        ]);
        let r = session(&p, 1, START);
        let line = r.last().unwrap();
        assert!(
            line.contains("roughly 9 more sessions to reach 27 WPM"),
            "{line}"
        );
        calm(&r);
    }

    #[test]
    fn a_flat_trend_is_normal() {
        let p = progress(vec![
            run(START - 6 * HOUR, "real", 17.0, 96.0),
            run(START - 3 * HOUR, "real", 17.0, 97.0),
            run(START + 60_000, "real", 16.5, 96.0),
        ]);
        let r = session(&p, 1, START);
        let line = r.last().unwrap();
        assert!(line.contains("No upward real-text trend yet"), "{line}");
        assert!(line.contains("normal"), "{line}");
        calm(&r);
    }
}
