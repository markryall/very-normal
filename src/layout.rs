//! The plan: the ladder of stages from where you start to your own layout.
//!
//! A plan is a TOML file (see `examples/very-normal.toml`). Positions run
//! left to right, top row then home row then bottom row, ten keys a row (five
//! per hand). Only letters and `; , . / '` live here; the outer columns never
//! move.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use anyhow::{Context, Result, bail};
use serde::Deserialize;

/// Mark's plan, used when you haven't made your own.
pub const EXAMPLE: &str = include_str!("../examples/very-normal.toml");

pub const OUTER_LEFT: [&str; 3] = ["tab", "esc", "shf"];
pub const OUTER_RIGHT: [&str; 3] = ["bsp", "'", "ent"];

/// The outer key on the right of a row. Beside the home row it's the quote,
/// unless the plan's 30 keys take the quote in (as Dvorak's do); then it
/// holds whatever they leave out.
pub fn outer_right(row: usize) -> String {
    if row == 1 {
        let stage0: String = keys(0).into_iter().collect();
        return crate::named::left_out(&stage0).to_string();
    }
    OUTER_RIGHT[row].to_string()
}

#[derive(Debug, Clone, Deserialize)]
pub struct Plan {
    pub name: String,
    #[serde(default)]
    pub oryx_layout: Option<String>,
    /// What each home row key does when held, left to right.
    #[serde(default)]
    pub home_row_mods: Vec<String>,
    #[serde(default)]
    pub common_words: Vec<String>,
    #[serde(rename = "stage")]
    pub stages: Vec<Stage>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Stage {
    pub name: String,
    #[serde(default)]
    pub blurb: String,
    pub keys: String,
    /// Share of your typing on the home row at this stage.
    #[serde(default)]
    pub home: f64,
    /// Share of letter pairs typed with the same finger at this stage.
    #[serde(default)]
    pub same_finger: f64,
    #[serde(default)]
    pub vim: Vec<String>,
    #[serde(default)]
    pub words: Vec<String>,
}

impl Plan {
    pub fn parse(text: &str) -> Result<Plan> {
        let mut plan: Plan = toml::from_str(text)?;
        for stage in &mut plan.stages {
            stage.keys.retain(|c| !c.is_whitespace());
        }
        plan.validate()?;
        Ok(plan)
    }

    pub fn load(path: &Path) -> Result<Plan> {
        let text =
            std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
        Plan::parse(&text).with_context(|| format!("in {}", path.display()))
    }

    fn validate(&self) -> Result<()> {
        if self.stages.len() < 2 {
            bail!("a plan needs at least two stages: where you start and where you're going");
        }
        let mut start: Vec<char> = self.stages[0].keys.chars().collect();
        start.sort_unstable();
        for (i, stage) in self.stages.iter().enumerate() {
            let mut k: Vec<char> = stage.keys.chars().collect();
            if k.len() != 30 {
                bail!(
                    "stage {i} has {} keys; every stage has 30 (three rows of ten)",
                    k.len()
                );
            }
            k.sort_unstable();
            if k != start {
                bail!("stage {i} isn't a rearrangement of stage 0's keys");
            }
            if i > 0 && stage.keys == self.stages[i - 1].keys {
                bail!("stage {i} doesn't move any keys");
            }
        }
        if !self.home_row_mods.is_empty() && self.home_row_mods.len() != 10 {
            bail!(
                "home_row_mods has {} entries; it needs 10, one per home row key",
                self.home_row_mods.len()
            );
        }
        if self.common_words.len() < 50 {
            bail!(
                "common_words needs at least 50 words for the drills; it has {}",
                self.common_words.len()
            );
        }
        for m in &self.home_row_mods {
            if !matches!(m.as_str(), "" | "ctrl" | "alt" | "cmd" | "shift") {
                bail!("home_row_mods can hold ctrl, alt, cmd, shift or \"\", not {m:?}");
            }
        }
        Ok(())
    }
}

/// `$VERY_NORMAL_PLAN`, or `~/.config/very-normal/plan.toml`.
pub fn plan_path() -> PathBuf {
    if let Some(p) = std::env::var_os("VERY_NORMAL_PLAN") {
        return PathBuf::from(p);
    }
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_default();
    home.join(".config/very-normal/plan.toml")
}

static PLAN: OnceLock<Plan> = OnceLock::new();

/// Loads your plan if you have one, otherwise Mark's. Call once at startup;
/// says which file it used, or `None` for the built-in example.
pub fn install() -> Result<Option<PathBuf>> {
    let path = plan_path();
    let (plan, used) = if path.exists() {
        (Plan::load(&path)?, Some(path))
    } else {
        (example(), None)
    };
    let _ = PLAN.set(plan);
    Ok(used)
}

fn example() -> Plan {
    Plan::parse(EXAMPLE).expect("the example plan is valid")
}

pub fn plan() -> &'static Plan {
    PLAN.get_or_init(example)
}

pub fn last() -> usize {
    plan().stages.len() - 1
}

fn stage(s: usize) -> &'static Stage {
    &plan().stages[s.min(last())]
}

pub fn name(s: usize) -> &'static str {
    &stage(s).name
}

pub fn blurb(s: usize) -> &'static str {
    &stage(s).blurb
}

/// Home row share and same-finger pairs, both as percentages.
pub fn stats(s: usize) -> (f64, f64) {
    (stage(s).home, stage(s).same_finger)
}

pub fn vim(s: usize) -> &'static [String] {
    &stage(s).vim
}

pub fn words(s: usize) -> &'static [String] {
    &stage(s).words
}

pub fn common_words() -> &'static [String] {
    &plan().common_words
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Change {
    pub index: usize,
    pub from: char,
    pub to: char,
}

impl Change {
    pub fn right_hand(&self) -> bool {
        self.index % 10 >= 5
    }
}

pub fn keys(s: usize) -> Vec<char> {
    stage(s).keys.chars().collect()
}

pub fn keys_string(s: usize) -> &'static str {
    &stage(s).keys
}

/// The keys that change when you climb from `stage - 1` to `stage`.
pub fn changes(s: usize) -> Vec<Change> {
    if s == 0 || s > last() {
        return Vec::new();
    }
    let before = keys(s - 1);
    keys(s)
        .into_iter()
        .enumerate()
        .filter(|(i, c)| before[*i] != *c)
        .map(|(index, to)| Change {
            index,
            from: before[index],
            to,
        })
        .collect()
}

/// The characters a stage puts somewhere new, in key order.
pub fn focus(s: usize) -> Vec<char> {
    let mut out = Vec::new();
    for c in changes(s) {
        if !out.contains(&c.to) {
            out.push(c.to);
        }
    }
    out
}

/// Whether a position types something different from stage 0.
pub fn moved(s: usize, index: usize) -> bool {
    keys(s)[index] != keys(0)[index]
}

pub fn changed_now(s: usize, index: usize) -> bool {
    changes(s).iter().any(|c| c.index == index)
}

/// What a key does when held, for home row positions 10 to 19.
pub fn modifier(index: usize) -> Option<&'static str> {
    let m = plan().home_row_mods.get(index.checked_sub(10)?)?;
    (!m.is_empty()).then_some(m.as_str())
}

pub fn position(s: usize, ch: char) -> Option<usize> {
    keys(s).iter().position(|c| *c == ch)
}

/// The left-hand position that mirrors a right-hand one, and the reverse.
pub fn mirror(index: usize) -> usize {
    let (row, col) = (index / 10, index % 10);
    row * 10 + (9 - col)
}

/// Which stage a main layer's 30 letter keys match, if any.
pub fn find_stage(main: &str) -> Option<usize> {
    plan().stages.iter().position(|s| s.keys == main)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_example_is_marks_very_normal() {
        let p = plan();
        assert_eq!(p.name, "Very Normal");
        assert_eq!(last(), 9);
        assert_eq!(keys_string(0), "qwertyuiopasdfghjkl;zxcvbnm,./");
        assert_eq!(keys_string(last()), "qwydgbhaujilstmpnoerzxcvk;f,./");
        assert_eq!(modifier(13), Some("cmd"));
        assert_eq!(modifier(10), None);
        assert_eq!(modifier(3), None);
    }

    #[test]
    fn every_stage_moves_a_few_keys_and_keeps_the_shortcuts() {
        for s in 1..=last() {
            let n = changes(s).len();
            assert!((3..=5).contains(&n), "stage {s} moves {n}");
            for c in "qwzxcv".chars() {
                assert_eq!(position(s, c), position(0, c), "{c} moved in stage {s}");
            }
        }
    }

    #[test]
    fn stage_one_swaps_e_l_t_f() {
        let got: Vec<(char, char)> = changes(1).iter().map(|c| (c.from, c.to)).collect();
        assert_eq!(got, vec![('e', 'l'), ('t', 'f'), ('f', 't'), ('l', 'e')]);
        assert_eq!(focus(1), vec!['l', 'f', 't', 'e']);
    }

    #[test]
    fn mirror_reflects_across_the_hands() {
        assert_eq!(mirror(18), 11);
        assert_eq!(mirror(0), 9);
        assert_eq!(mirror(mirror(23)), 23);
    }

    #[test]
    fn find_stage_recognises_each_stage() {
        for s in 0..=last() {
            assert_eq!(find_stage(keys_string(s)), Some(s));
        }
        assert_eq!(find_stage("nope"), None);
    }

    fn plan_with(stages: &[&str], extra: &str) -> Result<Plan> {
        let mut t = format!("name = \"t\"\n{extra}\n");
        t.push_str("common_words = [");
        for i in 0..60 {
            t.push_str(&format!("\"w{i}\", "));
        }
        t.push_str("]\n");
        for (i, k) in stages.iter().enumerate() {
            t.push_str(&format!("[[stage]]\nname = \"s{i}\"\nkeys = \"{k}\"\n"));
        }
        Plan::parse(&t)
    }

    #[test]
    fn rejects_broken_plans() {
        let q = "qwertyuiop asdfghjkl; zxcvbnm,./";
        let swap = "qwertyuiop asdfghjkl; zxcvbmn,./";
        assert!(plan_with(&[q, swap], "").is_ok());
        assert!(
            plan_with(&[q], "")
                .unwrap_err()
                .to_string()
                .contains("two stages")
        );
        assert!(
            plan_with(&[q, q], "")
                .unwrap_err()
                .to_string()
                .contains("doesn't move")
        );
        assert!(
            plan_with(&[q, "qwertyuiop"], "")
                .unwrap_err()
                .to_string()
                .contains("30")
        );
        assert!(
            plan_with(&[q, "qwertyuiop asdfghjkl; zxcvbnm,.!"], "")
                .unwrap_err()
                .to_string()
                .contains("rearrangement")
        );
        assert!(plan_with(&[q, swap], "home_row_mods = [\"hyper\"]").is_err());
    }
}
