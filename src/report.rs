//! Plain-text explanations shared by the command line and the TUI.

use crate::layout;
use crate::oryx::Snapshot;

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
}
