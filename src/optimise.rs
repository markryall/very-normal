//! Makes a plan from your own typing: scores layouts on what you type,
//! searches for a good one with simulated annealing, splits the move from
//! QWERTY into small stages, and picks drill words from your vocabulary.
//!
//! Finding the true best layout is a quadratic assignment problem, and
//! "best" depends on how the scoring is weighted anyway. Many layouts tie, so
//! each restart of the search lands somewhere slightly different; the summary
//! says how much they agreed.

use std::collections::HashMap;
use std::fmt::Write as _;
use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use serde_json::json;

use crate::named;
use crate::{corpus, layout, oryx, plans, store};

/// Every character the 30 keys can hold; one is always left out of a layout.
const CHARS: &[u8; 31] = b"abcdefghijklmnopqrstuvwxyz;,./'";
/// The position of the character a layout leaves out (on an outer key).
const OFF: usize = 30;
/// What typing the left-out character costs: an outer pinky key.
const OFF_EFFORT: f64 = 2.2;
/// Column to finger: 0-3 left, 6-9 right.
const FINGER: [usize; 10] = [0, 1, 2, 3, 3, 6, 6, 7, 8, 9];
const EFFORT: [[f64; 10]; 3] = [
    [2.2, 1.6, 1.3, 1.4, 2.0, 2.0, 1.4, 1.3, 1.6, 2.2],
    [1.6, 1.2, 1.0, 1.0, 1.6, 1.6, 1.0, 1.0, 1.2, 1.6],
    [2.0, 1.8, 1.7, 1.5, 2.2, 2.2, 1.5, 1.7, 1.8, 2.0],
];
/// Fewer messages than this is too little to optimise for.
pub const ENOUGH: usize = 200;
const ISSUES: &str = "https://github.com/markryall/very-normal/issues/new";

// ------------------------------------------------------------------ options

#[derive(Debug, Clone)]
pub struct Options {
    pub name: String,
    /// Read what you typed to Claude Code.
    pub claude: bool,
    pub projects: PathBuf,
    /// Other writing: files, folders, or `-` for stdin.
    pub text: Vec<String>,
    /// The layout you type on now: a known name, or your 30 keys.
    pub from: String,
    /// Keys that stay where they are; by default Q, W, Z, X, C, V and punctuation.
    pub lock: Option<String>,
    pub oryx_layout: Option<String>,
    pub home_row_mods: Option<Vec<String>>,
    pub per_stage: usize,
    pub restarts: usize,
    pub iters: usize,
    pub seed: u64,
    /// Where the plan goes; by default `plans/<name>.toml` next to your plan.
    pub out: Option<PathBuf>,
    /// Where the summary goes; by default next to the plan.
    pub summary: Option<PathBuf>,
    pub force: bool,
    pub corpus_out: Option<PathBuf>,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            name: "My Layout".into(),
            claude: true,
            projects: corpus::claude_projects(),
            text: Vec::new(),
            from: "QWERTY".into(),
            lock: None,
            oryx_layout: None,
            home_row_mods: None,
            per_stage: 2,
            restarts: 20,
            iters: 200_000,
            seed: 7,
            out: None,
            summary: None,
            force: false,
            corpus_out: None,
        }
    }
}

pub const USAGE: &str = "\
usage: very-normal make-plan [--name NAME] [--from LAYOUT] [--text FILE|FOLDER|- ...]
                             [--no-claude] [--lock KEYS] [--oryx-layout ID] [--home-row-mods M,M,...]
                             [--per-stage N] [--restarts N] [--iters N] [--seed N]
                             [--out FILE] [--summary FILE] [--force] [--corpus-out FILE]

Makes a layout from your own typing and a plan to get there from the layout
you type on now.
By default it reads what you typed to Claude Code (never Claude's replies).
--text adds any other writing: files, folders of .md/.txt/.org files, or -
for stdin. --no-claude leaves your Claude Code messages out.

--from         the layout you type on now: QWERTY (the default), Dvorak, Colemak,
               Colemak-DH, Workman or Norman, or your own 30 keys written top
               row first, like 'qwertyuiop asdfghjkl; zxcvbnm,./'
--lock         keys that stay where they are now (default: Q, W, Z, X, C and V for
               their Cmd shortcuts, and , . / '). Each one costs the layout a
               little; --lock '' locks nothing.
--oryx-layout  your Oryx layout id; its home row holds are read from Oryx
--out          the plan (default ~/.config/very-normal/plans/<name>.toml; private)
--summary      the shareable summary (default next to the plan, as .md)
--force        replace a plan you made before with the same name

It never touches the plan you're practising, or your progress. Your first
plan becomes the active one; after that, switch with `very-normal use <name>`.
";

impl Options {
    pub fn parse(args: &[&str]) -> Result<Options> {
        let mut o = Options::default();
        let mut i = 0;
        let value = |i: &mut usize, flag: &str| -> Result<String> {
            *i += 1;
            args.get(*i)
                .map(|s| s.to_string())
                .with_context(|| format!("{flag} needs a value"))
        };
        while i < args.len() {
            let flag = args[i];
            match flag {
                "--name" => o.name = value(&mut i, flag)?,
                "--no-claude" => o.claude = false,
                "--projects" => o.projects = value(&mut i, flag)?.into(),
                "--text" => {
                    let start = o.text.len();
                    while let Some(a) = args.get(i + 1).filter(|a| !a.starts_with("--")) {
                        o.text.push(a.to_string());
                        i += 1;
                    }
                    if o.text.len() == start {
                        bail!("--text needs at least one file or folder");
                    }
                }
                "--from" => o.from = value(&mut i, flag)?,
                "--lock" => o.lock = Some(value(&mut i, flag)?),
                "--oryx-layout" => o.oryx_layout = Some(value(&mut i, flag)?),
                "--home-row-mods" => {
                    let mods: Vec<String> = value(&mut i, flag)?
                        .split(',')
                        .map(|m| m.trim().to_string())
                        .collect();
                    if mods.len() != 10 {
                        bail!("--home-row-mods needs ten comma-separated entries");
                    }
                    o.home_row_mods = Some(mods);
                }
                "--per-stage" => o.per_stage = value(&mut i, flag)?.parse()?,
                "--restarts" => o.restarts = value(&mut i, flag)?.parse()?,
                "--iters" => o.iters = value(&mut i, flag)?.parse()?,
                "--seed" => o.seed = value(&mut i, flag)?.parse()?,
                "--out" => o.out = Some(value(&mut i, flag)?.into()),
                "--summary" => o.summary = Some(value(&mut i, flag)?.into()),
                "--force" => o.force = true,
                "--corpus-out" => o.corpus_out = Some(value(&mut i, flag)?.into()),
                other => bail!("make-plan doesn't know {other}\n\n{USAGE}"),
            }
            i += 1;
        }
        if !o.claude && o.text.is_empty() {
            bail!("--no-claude leaves nothing to read; add --text with some of your writing");
        }
        named::start(&o.from)?;
        if o.per_stage == 0 || o.restarts == 0 || o.iters == 0 {
            bail!("--per-stage, --restarts and --iters must be at least 1");
        }
        Ok(o)
    }
}

// ------------------------------------------------------------------ random

/// SplitMix64: small, seedable, and good enough for a search.
#[derive(Debug, Clone)]
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Rng {
        Rng(seed)
    }

    fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    fn below(&mut self, n: usize) -> usize {
        (self.next_u64() % n as u64) as usize
    }

    fn unit(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }

    fn shuffle<T>(&mut self, items: &mut [T]) {
        for i in (1..items.len()).rev() {
            items.swap(i, self.below(i + 1));
        }
    }
}

// ------------------------------------------------------------------ scoring

/// How often each key, and each pair of keys in a row, was typed.
#[derive(Debug, Clone)]
pub struct Counts {
    uni: [f64; 31],
    bi: [[f64; 31]; 31],
}

fn index(ch: char) -> Option<usize> {
    CHARS.iter().position(|c| *c as char == ch)
}

impl Counts {
    /// Counts lowercase text; anything that isn't one of the 31 characters
    /// breaks a pair.
    pub fn of(text: &str) -> Counts {
        let mut c = Counts {
            uni: [0.0; 31],
            bi: [[0.0; 31]; 31],
        };
        let mut prev: Option<usize> = None;
        for ch in text.chars() {
            let Some(i) = index(ch) else {
                prev = None;
                continue;
            };
            c.uni[i] += 1.0;
            if let Some(p) = prev
                && p != i
            {
                c.bi[p][i] += 1.0;
            }
            prev = Some(i);
        }
        c
    }

    fn total(&self) -> f64 {
        self.uni.iter().sum::<f64>().max(1.0)
    }
}

/// The cost of each key, and of each pair of keys typed one after the other.
/// Position `OFF` is the outer key holding the character a layout leaves out:
/// it costs `OFF_EFFORT`, and pairs with it cost nothing either way.
struct Model {
    effort: [f64; 31],
    pair: [[f64; 31]; 31],
}

fn row_col(p: usize) -> (usize, usize) {
    (p / 10, p % 10)
}

impl Model {
    fn new() -> Model {
        let mut m = Model {
            effort: [0.0; 31],
            pair: [[0.0; 31]; 31],
        };
        m.effort[OFF] = OFF_EFFORT;
        for p in 0..30 {
            let (r1, c1) = row_col(p);
            m.effort[p] = EFFORT[r1][c1];
            for q in 0..30 {
                if p == q {
                    continue;
                }
                let (r2, c2) = row_col(q);
                let (f1, f2) = (FINGER[c1], FINGER[c2]);
                let rows = r1.abs_diff(r2) as f64;
                m.pair[p][q] = if f1 == f2 {
                    // Same finger, different key: the slow, clunky pairs.
                    6.0 * (1.0 + 0.5 * rows) + if c1 != c2 { 1.0 } else { 0.0 }
                } else if (f1 < 5) == (f2 < 5) {
                    if f1.abs_diff(f2) == 1 && r1.abs_diff(r2) == 2 {
                        2.0 // scissor
                    } else if r1 == r2 {
                        // A roll along one row: inward is nicer.
                        let inward = if f1 < 5 { f2 > f1 } else { f2 < f1 };
                        if inward { -0.4 } else { -0.2 }
                    } else {
                        0.0
                    }
                } else {
                    0.0
                };
            }
        }
        m
    }

    /// Total cost, not yet divided by how much was typed.
    fn raw(&self, p: &Positions, c: &Counts) -> f64 {
        let mut s = 0.0;
        for i in 0..31 {
            s += c.uni[i] * self.effort[p[i]];
            for j in 0..31 {
                s += c.bi[i][j] * self.pair[p[i]][p[j]];
            }
        }
        s
    }

    /// The part of the raw cost that involves keys `a` or `b`.
    fn part(&self, p: &Positions, c: &Counts, a: usize, b: usize) -> f64 {
        let mut s = c.uni[a] * self.effort[p[a]] + c.uni[b] * self.effort[p[b]];
        for j in 0..31 {
            s += c.bi[a][j] * self.pair[p[a]][p[j]] + c.bi[j][a] * self.pair[p[j]][p[a]];
            s += c.bi[b][j] * self.pair[p[b]][p[j]] + c.bi[j][b] * self.pair[p[j]][p[b]];
        }
        s - (c.bi[a][b] * self.pair[p[a]][p[b]] + c.bi[b][a] * self.pair[p[b]][p[a]])
    }

    /// How the raw cost changes if keys `a` and `b` trade places.
    fn swap_delta(&self, p: &mut Positions, c: &Counts, a: usize, b: usize) -> f64 {
        let before = self.part(p, c, a, b);
        p.swap(a, b);
        let after = self.part(p, c, a, b);
        p.swap(a, b);
        after - before
    }
}

/// Where each character (in `CHARS` order) sits: 0-9 top row, 10-19 home
/// row, 20-29 bottom row, or `OFF` for the one the layout leaves out.
type Positions = [usize; 31];

fn positions(keys: &str) -> Positions {
    let keys = keys.as_bytes();
    std::array::from_fn(|i| keys.iter().position(|k| *k == CHARS[i]).unwrap_or(OFF))
}

fn keys_of(p: &Positions) -> String {
    let mut grid = [b' '; 30];
    for (i, pos) in p.iter().enumerate() {
        if *pos != OFF {
            grid[*pos] = CHARS[i];
        }
    }
    String::from_utf8(grid.to_vec()).expect("ascii")
}

#[derive(Debug, Clone, Copy)]
pub struct Metrics {
    /// Effort per key typed; lower is better.
    pub score: f64,
    /// Percentage of keys typed on the home row.
    pub home: f64,
    /// Percentage of key pairs typed with the same finger.
    pub same_finger: f64,
    /// Percentage of keys typed with a pinky.
    pub pinky: f64,
}

fn metrics(model: &Model, keys: &str, c: &Counts) -> Metrics {
    let p = positions(keys);
    let total = c.total();
    // The left-out character is on an outer key, under the pinky.
    let finger = |i: usize| if p[i] == OFF { 9 } else { FINGER[p[i] % 10] };
    let (mut home, mut pinky, mut same, mut pairs) = (0.0, 0.0, 0.0, 0.0);
    for (i, pos) in p.iter().enumerate() {
        if *pos != OFF && pos / 10 == 1 {
            home += c.uni[i];
        }
        if matches!(finger(i), 0 | 9) {
            pinky += c.uni[i];
        }
        for j in 0..31 {
            pairs += c.bi[i][j];
            if finger(i) == finger(j) && p[i] != OFF && p[j] != OFF {
                same += c.bi[i][j];
            }
        }
    }
    Metrics {
        score: model.raw(&p, c) / total,
        home: 100.0 * home / total,
        same_finger: 100.0 * same / pairs.max(1.0),
        pinky: 100.0 * pinky / total,
    }
}

/// Simulated annealing from a shuffle of where you start, with locked keys
/// pinned.
fn anneal(
    model: &Model,
    c: &Counts,
    start: &str,
    lock: &str,
    iters: usize,
    rng: &mut Rng,
) -> (f64, Positions) {
    let mut p = positions(start);
    let free: Vec<usize> = (0..31)
        .filter(|i| p[*i] != OFF && !lock.contains(CHARS[*i] as char))
        .collect();
    if free.len() < 2 {
        return (model.raw(&p, c) / c.total(), p);
    }
    let mut spots: Vec<usize> = free.iter().map(|i| p[*i]).collect();
    rng.shuffle(&mut spots);
    for (i, spot) in free.iter().zip(spots) {
        p[*i] = spot;
    }
    let total = c.total();
    let mut cur = model.raw(&p, c) / total;
    let mut best = (cur, p);
    for k in 0..iters {
        let t = 0.0005f64.powf(k as f64 / iters as f64);
        let a = free[rng.below(free.len())];
        let mut b = free[rng.below(free.len() - 1)];
        if b == a {
            b = free[free.len() - 1];
        }
        let delta = model.swap_delta(&mut p, c, a, b) / total;
        if delta < 0.0 || rng.unit() < (-delta / t).exp() {
            p.swap(a, b);
            cur += delta;
            if cur < best.0 {
                best = (cur, p);
            }
        }
    }
    (model.raw(&best.1, c) / total, best.1)
}

// ------------------------------------------------------------------ stages

/// Swaps one key into its final place at a time, most-typed first, and
/// groups the swaps into stages.
fn ladder(start: &str, target: &str, freq: &[usize; 31], per_stage: usize) -> Vec<String> {
    let mut order: Vec<usize> = (0..31)
        .filter(|i| start.contains(CHARS[*i] as char))
        .collect();
    order.sort_by_key(|i| std::cmp::Reverse(freq[*i]));
    let mut state = start.as_bytes().to_vec();
    let target_bytes = target.as_bytes();
    let mut swaps = Vec::new();
    for i in order {
        let ch = CHARS[i];
        let want = target_bytes.iter().position(|c| *c == ch).expect("key");
        let have = state.iter().position(|c| *c == ch).expect("key");
        if want != have {
            let other = state[want];
            state.swap(want, have);
            swaps.push((ch, other));
        }
    }
    debug_assert_eq!(state, target_bytes);
    let mut out = vec![start.to_string()];
    let mut state = start.as_bytes().to_vec();
    for group in swaps.chunks(per_stage) {
        for (a, b) in group {
            let i = state.iter().position(|c| c == a).expect("key");
            let j = state.iter().position(|c| c == b).expect("key");
            state.swap(i, j);
        }
        out.push(String::from_utf8(state.clone()).expect("ascii"));
    }
    out
}

fn upper_list(chars: &[char]) -> String {
    chars
        .iter()
        .map(|c| c.to_ascii_uppercase().to_string())
        .collect::<Vec<_>>()
        .join(" and ")
}

/// Names each stage after the letters that reach their final key in it.
fn names_and_blurbs(
    model: &Model,
    plan_name: &str,
    ladder: &[String],
    c: &Counts,
) -> Vec<(String, String)> {
    let target = ladder.last().expect("a ladder has stages").as_bytes();
    let mut out = vec![(
        "Baseline".to_string(),
        "Your layout as it is today. Record three 60-second real-text runs so later stages have numbers to beat.".to_string(),
    )];
    for s in 1..ladder.len() {
        let (before, after) = (ladder[s - 1].as_bytes(), ladder[s].as_bytes());
        let mut moved: Vec<char> = (0..30)
            .filter(|i| before[*i] != after[*i] && after[*i].is_ascii_alphabetic())
            .map(|i| after[i] as char)
            .collect();
        moved.sort_unstable();
        moved.dedup();
        let at = |keys: &[u8], ch: char| keys.iter().position(|k| *k as char == ch);
        let mut settled: Vec<char> = moved
            .iter()
            .copied()
            .filter(|ch| at(after, *ch) == at(target, *ch))
            .collect();
        if settled.is_empty() {
            settled = moved.clone();
        }
        let home: Vec<char> = settled
            .iter()
            .copied()
            .filter(|ch| at(after, *ch).is_some_and(|p| p / 10 == 1))
            .collect();
        let name = if s == ladder.len() - 1 {
            format!("Fully {plan_name}")
        } else if !home.is_empty() {
            let two = &home[..home.len().min(2)];
            format!(
                "{} {}",
                upper_list(two),
                if two.len() > 1 {
                    "come home"
                } else {
                    "comes home"
                }
            )
        } else if !settled.is_empty() {
            let two = &settled[..settled.len().min(2)];
            format!(
                "{} {}",
                upper_list(two),
                if two.len() > 1 { "settle" } else { "settles" }
            )
        } else {
            format!("Stage {s}")
        };
        let m = metrics(model, &ladder[s], c);
        let blurb = format!(
            "{} move. {:.0}% of your typing lands on the home row and {:.1}% of letter pairs use the same finger.",
            moved
                .iter()
                .map(|c| c.to_ascii_uppercase().to_string())
                .collect::<Vec<_>>()
                .join(", "),
            m.home,
            m.same_finger
        );
        out.push((name, blurb));
    }
    out
}

// ------------------------------------------------------------------ words

/// Lowercase dictionary words, and the names to keep out of drills.
fn dictionary() -> (
    std::collections::HashSet<String>,
    std::collections::HashSet<String>,
) {
    let read = |p: &str| std::fs::read_to_string(p).unwrap_or_default();
    let words = ["/usr/share/dict/web2", "/usr/share/dict/words"]
        .iter()
        .map(|p| read(p))
        .find(|t| !t.is_empty())
        .unwrap_or_default()
        .lines()
        .filter(|w| w.starts_with(|c: char| c.is_lowercase()))
        .map(|w| w.trim().to_string())
        .collect();
    let small: std::collections::HashSet<String> = read("/usr/share/dict/connectives")
        .lines()
        .map(|w| w.trim().to_lowercase())
        .collect();
    let names = read("/usr/share/dict/propernames")
        .lines()
        .map(|w| w.trim().to_lowercase())
        .filter(|w| !small.contains(w))
        .collect();
    (words, names)
}

/// Words like `don't`: letters, optionally an apostrophe and more letters.
fn tokens(text: &str) -> Vec<&str> {
    let b = text.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        if !b[i].is_ascii_alphabetic() {
            i += 1;
            continue;
        }
        let start = i;
        while i < b.len() && b[i].is_ascii_alphabetic() {
            i += 1;
        }
        if i + 1 < b.len() && b[i] == b'\'' && b[i + 1].is_ascii_alphabetic() {
            i += 1;
            while i < b.len() && b[i].is_ascii_alphabetic() {
                i += 1;
            }
        }
        out.push(&text[start..i]);
    }
    out
}

/// Your words, most used first: dictionary words only, no names, and none
/// you mostly write with a capital (which is how names look).
fn vocabulary(raw: &str) -> Vec<(String, usize)> {
    let (words, names) = dictionary();
    let mut count: HashMap<String, (usize, usize, usize)> = HashMap::new(); // (n, capitalised, first seen)
    for (k, t) in tokens(raw).into_iter().enumerate() {
        let lower = t.to_ascii_lowercase();
        let e = count.entry(lower).or_insert((0, 0, k));
        e.0 += 1;
        if t.starts_with(|c: char| c.is_ascii_uppercase()) {
            e.1 += 1;
        }
    }
    let ok = |w: &str, capitals: usize, n: usize| {
        let (base, tail) = w.split_once('\'').unwrap_or((w, ""));
        if names.contains(base) || (!words.is_empty() && !words.contains(base)) {
            return false;
        }
        if !matches!(tail, "" | "s" | "t" | "d" | "ll" | "re" | "ve" | "m") {
            return false;
        }
        if base != "i" && capitals * 2 > n {
            return false;
        }
        w.len() > 1 || w == "a" || w == "i"
    };
    let mut out: Vec<(String, usize, usize)> = count
        .into_iter()
        .filter(|(w, (n, caps, _))| *n >= 3 && ok(w, *caps, *n))
        .map(|(w, (n, _, first))| (w, n, first))
        .collect();
    out.sort_by(|a, b| b.1.cmp(&a.1).then(a.2.cmp(&b.2)));
    out.into_iter().map(|(w, n, _)| (w, n)).collect()
}

fn stage_words(vocab: &[(String, usize)], focus: &[char]) -> Vec<String> {
    let mut scored: Vec<(f64, &str)> = vocab
        .iter()
        .filter(|(w, n)| w.len() >= 3 && *n >= 4 && w.chars().any(|c| focus.contains(&c)))
        .map(|(w, n)| {
            let hits = w.chars().filter(|c| focus.contains(c)).count() as f64;
            let s = hits / (w.len() as f64).sqrt() * (*n.min(&50) as f64).powf(0.3);
            (s, w.as_str())
        })
        .collect();
    scored.sort_by(|a, b| b.0.total_cmp(&a.0).then(b.1.cmp(a.1)));
    scored
        .into_iter()
        .take(80)
        .map(|(_, w)| w.to_string())
        .collect()
}

/// Your own words that use only your new home row: fun layout names.
fn home_row_words(vocab: &[(String, usize)], target: &str) -> Vec<String> {
    let home = &target[10..20];
    let mut found: Vec<&(String, usize)> = vocab
        .iter()
        .filter(|(w, _)| w.len() >= 4 && w.chars().all(|c| home.contains(c)))
        .collect();
    found.sort_by(|a, b| b.0.len().cmp(&a.0.len()).then(b.1.cmp(&a.1)));
    found.into_iter().take(8).map(|(w, _)| w.clone()).collect()
}

// ------------------------------------------------------------------ output

fn toml_list(items: &[String]) -> String {
    let mut lines = Vec::new();
    let mut line = String::from("  ");
    for x in items {
        let piece = format!("{}, ", serde_json::to_string(x).expect("string"));
        if line.len() + piece.len() > 88 {
            lines.push(line.trim_end().to_string());
            line = String::from("  ");
        }
        line.push_str(&piece);
    }
    lines.push(line.trim_end().trim_end_matches(',').to_string());
    format!("[\n{}\n]", lines.join("\n"))
}

fn rows(keys: &str) -> String {
    format!("{} {} {}", &keys[..10], &keys[10..20], &keys[20..])
}

/// The layout as three rows split into two hands.
pub fn pretty(keys: &str) -> String {
    (0..3)
        .map(|r| {
            let row = &keys[r * 10..r * 10 + 10];
            let hand = |s: &str| s.chars().map(String::from).collect::<Vec<_>>().join(" ");
            format!("{}   {}", hand(&row[..5]), hand(&row[5..]))
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// What making a plan produced.
#[derive(Debug, Clone)]
pub struct Made {
    pub summary: String,
    pub target: String,
    pub stages: usize,
    pub name_ideas: Vec<String>,
    pub plan_path: PathBuf,
    pub summary_path: PathBuf,
    /// It became the plan you're practising, since you had none yet.
    pub activated: bool,
    pub slug: String,
}

/// Reads your writing according to the options: what was found, and where from.
pub fn gather(o: &Options) -> Result<(Vec<String>, Vec<&'static str>)> {
    let mut msgs = Vec::new();
    let mut sources = Vec::new();
    if o.claude {
        let typed = corpus::typed_messages(&o.projects);
        if !typed.is_empty() {
            sources.push("Claude Code");
        }
        msgs.extend(typed);
    }
    if !o.text.is_empty() {
        let text = corpus::text_messages(&o.text)?;
        if !text.is_empty() {
            sources.push("other writing");
        }
        msgs.extend(text);
    }
    Ok((msgs, sources))
}

/// Makes a plan and its summary, and writes both. Progress goes to stderr.
pub fn make(o: &Options) -> Result<Made> {
    let (start_name, start) = named::start(&o.from)?;
    let lock = o
        .lock
        .clone()
        .unwrap_or_else(|| named::default_lock(&start));
    if let Some(c) = lock.chars().find(|c| !start.contains(*c)) {
        bail!("--lock has {c:?}, which isn't one of {start_name}'s 30 keys");
    }
    let slug = plans::slug(&o.name);
    let progress = store::default_path();
    let fresh = plans::fresh(&progress);
    let out = o.out.clone().unwrap_or_else(|| plans::made_path(&o.name));
    let summary_out = o.summary.clone().unwrap_or_else(|| plans::summary_of(&out));
    if o.out.is_none() && !fresh {
        if slug == plans::active_slug() {
            bail!(
                "{} is the plan you're practising; give this one another --name",
                o.name
            );
        }
        if plans::parked(&progress, &slug).exists() {
            bail!(
                "you've practised a plan called {} and its progress is kept; give this one another --name",
                o.name
            );
        }
    }
    if out.exists() && !o.force {
        bail!(
            "you already made a plan at {}; pass --force to replace it, or give this one another --name",
            out.display()
        );
    }
    let mods = match (&o.home_row_mods, &o.oryx_layout) {
        (Some(m), _) => m.clone(),
        (None, Some(id)) => {
            let m = oryx::home_row_mods(id)
                .context("reading your home row holds from Oryx (or pass --home-row-mods)")?;
            eprintln!("Home row holds from Oryx: {}", m.join(","));
            m
        }
        (None, None) => Vec::new(),
    };

    let (msgs, sources) = gather(o)?;
    if msgs.len() < ENOUGH {
        bail!(
            "only {} messages or paragraphs found; that's too few to optimise for. Add more of your writing with --text",
            msgs.len()
        );
    }
    let raw = msgs.join("\n");
    let letters = raw.chars().filter(|c| c.is_alphabetic()).count();
    let upper =
        100.0 * raw.chars().filter(|c| c.is_uppercase()).count() as f64 / letters.max(1) as f64;
    let text = raw.to_lowercase();
    let characters = raw.chars().count();
    if let Some(path) = &o.corpus_out {
        std::fs::write(path, &raw).with_context(|| format!("writing {}", path.display()))?;
    }
    eprintln!(
        "{} messages, {characters} characters, {upper:.1}% uppercase",
        msgs.len()
    );

    let mut rng = Rng::new(o.seed);
    let mut shuffled = msgs.clone();
    rng.shuffle(&mut shuffled);
    let cut = shuffled.len() * 4 / 5;
    let train = Counts::of(&shuffled[..cut].join("\n").to_lowercase());
    let test = Counts::of(&shuffled[cut..].join("\n").to_lowercase());
    let full = Counts::of(&text);
    let model = Model::new();

    let results: Vec<(f64, Positions)> = std::thread::scope(|s| {
        let handles: Vec<_> = (0..o.restarts)
            .map(|r| {
                let (model, train, start, lock) = (&model, &train, start.as_str(), lock.as_str());
                let seed = o
                    .seed
                    .wrapping_add(1 + r as u64)
                    .wrapping_mul(0x2545_F491_4F6C_DD1D);
                s.spawn(move || anneal(model, train, start, lock, o.iters, &mut Rng::new(seed)))
            })
            .collect();
        handles
            .into_iter()
            .map(|h| h.join().expect("search thread"))
            .collect()
    });
    for (r, (score, p)) in results.iter().enumerate() {
        eprintln!(
            "restart {}/{}: {score:.4}  {}",
            r + 1,
            o.restarts,
            rows(&keys_of(p))
        );
    }
    let best = results
        .iter()
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .expect("at least one restart");
    let target = keys_of(&best.1);
    let restart_layouts: Vec<String> = results.iter().map(|(_, p)| keys_of(p)).collect();
    let agree = (0..30)
        .filter(|i| {
            restart_layouts
                .iter()
                .all(|l| l.as_bytes()[*i] == target.as_bytes()[*i])
        })
        .count();
    let distinct = {
        let mut l = restart_layouts.clone();
        l.sort();
        l.dedup();
        l.len()
    };

    let mut freq = [0usize; 31];
    for ch in text.chars() {
        if let Some(i) = index(ch) {
            freq[i] += 1;
        }
    }
    let steps = ladder(&start, &target, &freq, o.per_stage);
    let titles = names_and_blurbs(&model, &o.name, &steps, &full);
    let vocab = vocabulary(&raw);
    let common: Vec<String> = vocab.iter().take(500).map(|(w, _)| w.clone()).collect();
    if common.len() < 50 {
        bail!(
            "only {} different everyday words in your writing; the drills need at least 50. Add more with --text",
            common.len()
        );
    }

    // The plan: private, since the drill words are your vocabulary.
    let mut plan = String::new();
    let _ = writeln!(
        plan,
        "# A very-normal plan for {}, from {start_name}, made by `very-normal make-plan`\n# from your own writing. The drill words are your vocabulary: keep this file private.\n",
        o.name
    );
    let _ = writeln!(plan, "name = {}", json!(o.name));
    match &o.oryx_layout {
        Some(id) => {
            let _ = writeln!(plan, "oryx_layout = {}", json!(id));
        }
        None => plan.push_str(
            "# oryx_layout = \"abc12\"   # your Oryx layout id, if you have a ZSA board\n",
        ),
    }
    if mods.is_empty() {
        plan.push_str("# home_row_mods = [\"\", \"ctrl\", \"alt\", \"cmd\", \"\", \"\", \"cmd\", \"alt\", \"ctrl\", \"\"]\n");
    } else {
        let _ = writeln!(plan, "home_row_mods = {}", json!(mods));
    }
    let _ = writeln!(plan, "\ncommon_words = {}", toml_list(&common));
    for (s, keys) in steps.iter().enumerate() {
        let m = metrics(&model, keys, &full);
        let before = if s > 0 {
            steps[s - 1].as_bytes()
        } else {
            keys.as_bytes()
        };
        let mut focus: Vec<char> = (0..30)
            .filter(|i| before[*i] != keys.as_bytes()[*i])
            .map(|i| keys.as_bytes()[i] as char)
            .collect();
        focus.sort_unstable();
        let (name, blurb) = &titles[s];
        let _ = write!(
            plan,
            "\n[[stage]]\nname = {}\nblurb = {}\nkeys = {}\nhome = {:.0}\nsame_finger = {:.1}\nvim = {}\n",
            json!(name),
            json!(blurb),
            json!(rows(keys)),
            m.home,
            m.same_finger,
            json!(focus.iter().map(|c| c.to_string()).collect::<Vec<_>>()),
        );
        if s > 0 {
            let _ = writeln!(plan, "words = {}", toml_list(&stage_words(&vocab, &focus)));
        }
    }
    layout::Plan::parse(&plan).context("the plan it made doesn't load; that's a bug")?;

    let summary = summary(&SummaryInput {
        name: &o.name,
        target: &target,
        model: &model,
        test: &test,
        full: &full,
        freq: &freq,
        messages: msgs.len(),
        characters,
        upper,
        lock: &lock,
        start_name: &start_name,
        start: &start,
        sources: &sources,
        stages: steps.len() - 1,
        ladder: &steps,
        stage_names: &titles.iter().map(|(n, _)| n.as_str()).collect::<Vec<_>>(),
        restarts: &results,
        agree,
        distinct,
    });

    if let Some(dir) = out.parent() {
        std::fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
    }
    std::fs::write(&out, plan).with_context(|| format!("writing {}", out.display()))?;
    if let Some(dir) = summary_out.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(&summary_out, &summary)
        .with_context(|| format!("writing {}", summary_out.display()))?;
    let activated = o.out.is_none() && fresh;
    if activated {
        plans::activate(&out)?;
    }
    Ok(Made {
        summary,
        name_ideas: home_row_words(&vocab, &target),
        target,
        stages: steps.len() - 1,
        plan_path: out,
        summary_path: summary_out,
        activated,
        slug,
    })
}

struct SummaryInput<'a> {
    name: &'a str,
    target: &'a str,
    model: &'a Model,
    test: &'a Counts,
    full: &'a Counts,
    freq: &'a [usize; 31],
    messages: usize,
    characters: usize,
    upper: f64,
    lock: &'a str,
    /// The layout you type on now, and its name.
    start_name: &'a str,
    start: &'a str,
    sources: &'a [&'static str],
    stages: usize,
    /// Every stage's keys, where you start first, and what each is called.
    ladder: &'a [String],
    stage_names: &'a [&'a str],
    restarts: &'a [(f64, Positions)],
    agree: usize,
    distinct: usize,
}

/// The shareable summary: your layout and how it scores. No words, no text.
fn summary(s: &SummaryInput) -> String {
    let mut table = String::from(
        "| Layout | Effort (lower is better) | Home row | Same-finger pairs | Pinky |\n|---|---|---|---|---|\n",
    );
    let mut scores = serde_json::Map::new();
    let mut compared = vec![(s.name, s.target)];
    if !named::KNOWN.iter().any(|(_, k)| *k == s.start) {
        compared.push((s.start_name, s.start));
    }
    compared.extend(named::KNOWN.iter().copied());
    for (label, keys) in &compared {
        let m = metrics(s.model, keys, s.test);
        let now = if *keys == s.start && *label != s.name {
            " (what you type on now)"
        } else {
            ""
        };
        let _ = writeln!(
            table,
            "| {label}{now} | {:.3} | {:.0}% | {:.1}% | {:.1}% |",
            m.score, m.home, m.same_finger, m.pinky
        );
        let key = if *label == s.name { "yours" } else { label };
        scores.insert(
            key.to_string(),
            json!({
                "effort": round(m.score, 4),
                "home": round(m.home, 1),
                "same_finger": round(m.same_finger, 2),
                "pinky": round(m.pinky, 2),
            }),
        );
    }
    let mut order: Vec<usize> = (0..31).collect();
    order.sort_by_key(|i| std::cmp::Reverse(s.freq[*i]));
    let top: String = order[..10].iter().map(|i| CHARS[*i] as char).collect();
    let total: usize = s.freq.iter().sum::<usize>().max(1);
    let frequency: serde_json::Map<String, serde_json::Value> = (0..31)
        .map(|i| {
            (
                (CHARS[i] as char).to_string(),
                json!(round(100.0 * s.freq[i] as f64 / total as f64, 2)),
            )
        })
        .collect();
    let q = metrics(s.model, s.start, s.full);
    let moved = (0..30)
        .filter(|i| s.target.as_bytes()[*i] != s.start.as_bytes()[*i])
        .count();
    let mut runs: Vec<(f64, String)> = s.restarts.iter().map(|(c, p)| (*c, keys_of(p))).collect();
    runs.sort_by(|a, b| a.0.total_cmp(&b.0));
    let spread = runs.last().map_or(0.0, |w| w.0) - runs.first().map_or(0.0, |b| b.0);
    let data = json!({
        "format": 2,
        "name": s.name,
        "layout": rows(s.target),
        "start": { "name": s.start_name, "layout": rows(s.start) },
        "lock": s.lock,
        "sources": s.sources,
        "messages": s.messages,
        "characters": s.characters,
        "uppercase": round(s.upper, 1),
        "key_frequency": frequency,
        "scores": scores,
        "restarts": distinct_runs(&runs),
        "keys_every_restart_agreed_on": s.agree,
        "stages": s.ladder.iter().skip(1).map(|k| rows(k)).collect::<Vec<_>>(),
        "version": env!("CARGO_PKG_VERSION"),
    });
    format!(
        "# {name}\n\n```\n{pretty}\n```\n\n\
         - Typing studied: {messages} messages from {sources}, {chars} characters, {upper:.1}% uppercase\n\
         - Ten most-typed keys: `{top}`\n\
         - Locked in place: `{lock}`\n\
         - Starting from: {start_name}, `{start_rows}`\n\
         - Stages from {start_name}: {stages}, moving {moved} keys\n\
         - On {start_name}: {qh:.0}% home row, {qs:.1}% same-finger pairs\n\
         - The search ran {n} times and found {distinct} different layouts, {spread:.4} apart in effort. \
         All {n} put {agree} of the 30 keys in the same place.\n\n\
         Scored on 20% of messages the optimiser never saw:\n\n{table}\n\
         The stages from {start_name}, most-typed letters first:\n\n{ladder}\n\
         <details><summary>The numbers, for comparing layouts</summary>\n\n```json\n{json}\n```\n\n</details>\n\n\
         Made with [very-normal](https://github.com/markryall/very-normal) {version}.\n",
        name = s.name,
        pretty = pretty(s.target),
        messages = s.messages,
        sources = if s.sources.is_empty() {
            "nowhere".to_string()
        } else {
            s.sources.join(" and ")
        },
        chars = s.characters,
        upper = s.upper,
        lock = s.lock,
        start_name = s.start_name,
        start_rows = rows(s.start),
        stages = s.stages,
        qh = q.home,
        qs = q.same_finger,
        n = runs.len(),
        json = serde_json::to_string(&data).expect("json"),
        ladder = stage_table(s),
        version = env!("CARGO_PKG_VERSION"),
        distinct = s.distinct,
        agree = s.agree,
    )
}

/// Each stage's moves and how it does on your typing.
fn stage_table(s: &SummaryInput) -> String {
    let mut t = String::from(
        "| Stage | | Moves | Home row | Same-finger pairs | Keys |\n|---|---|---|---|---|---|\n",
    );
    for (i, keys) in s.ladder.iter().enumerate() {
        let m = metrics(s.model, keys, s.full);
        let moves = if i == 0 {
            String::new()
        } else {
            let before = s.ladder[i - 1].as_bytes();
            let mut moved: Vec<char> = (0..30)
                .filter(|k| before[*k] != keys.as_bytes()[*k])
                .map(|k| keys.as_bytes()[k] as char)
                .collect();
            moved.sort_unstable();
            moved
                .iter()
                .map(|c| c.to_string())
                .collect::<Vec<_>>()
                .join(" ")
        };
        let _ = writeln!(
            t,
            "| {i} | {} | {moves} | {:.0}% | {:.1}% | `{}` |",
            s.stage_names.get(i).copied().unwrap_or(""),
            m.home,
            m.same_finger,
            rows(keys)
        );
    }
    t
}

/// Each different layout the restarts found, best first, and how many found it.
fn distinct_runs(runs: &[(f64, String)]) -> Vec<serde_json::Value> {
    let mut out: Vec<(f64, &str, usize)> = Vec::new();
    for (c, l) in runs {
        match out.iter_mut().find(|(_, k, _)| *k == l) {
            Some(e) => e.2 += 1,
            None => out.push((*c, l, 1)),
        }
    }
    out.iter()
        .map(|(c, l, n)| json!({"effort": round(*c, 4), "layout": rows(l), "times": n}))
        .collect()
}

fn round(x: f64, places: i32) -> f64 {
    let f = 10f64.powi(places);
    (x * f).round() / f
}

// ------------------------------------------------------------------ sharing

/// A link that opens a new issue on very-normal with your summary filled in.
/// You see it, and can change it, before anything is posted.
pub fn share_url(summary: &str) -> String {
    let title = summary
        .lines()
        .next()
        .and_then(|l| l.strip_prefix("# "))
        .unwrap_or("My layout");
    format!(
        "{ISSUES}?title={}&body={}",
        encode(&format!("Layout: {title}")),
        encode(summary)
    )
}

/// Browsers and GitHub cope with links up to about this long.
pub const LONGEST_LINK: usize = 8000;

fn encode(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || b"-_.~,:;/()*!'@$".contains(&b) {
            out.push(b as char);
        } else {
            let _ = write!(out, "%{b:02X}");
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::named::QWERTY;

    fn dvorak() -> &'static str {
        named::find("dvorak").unwrap().1
    }

    const SAMPLE: &str = "the quick brown fox jumps over the lazy dog, then the dog sleeps. \
        we typed this sample so the scoring has something to chew on; it's fine.";

    #[test]
    fn swap_delta_matches_a_full_rescore() {
        let model = Model::new();
        let c = Counts::of(SAMPLE);
        let mut p = positions(QWERTY);
        let mut rng = Rng::new(3);
        for _ in 0..200 {
            let a = rng.below(30);
            let b = (a + 1 + rng.below(29)) % 30;
            let before = model.raw(&p, &c);
            let delta = model.swap_delta(&mut p, &c, a, b);
            p.swap(a, b);
            let after = model.raw(&p, &c);
            assert!((after - before - delta).abs() < 1e-9, "{a} {b}");
        }
    }

    #[test]
    fn annealing_beats_where_you_start_and_keeps_locked_keys() {
        let model = Model::new();
        let c = Counts::of(&SAMPLE.repeat(20));
        for start in [QWERTY, dvorak()] {
            let lock = named::default_lock(start);
            let (score, p) = anneal(&model, &c, start, &lock, 20_000, &mut Rng::new(1));
            let keys = keys_of(&p);
            assert!(score < metrics(&model, start, &c).score);
            for ch in lock.chars() {
                assert_eq!(keys.find(ch), start.find(ch), "{ch} moved");
            }
            let mut a: Vec<char> = keys.chars().collect();
            let mut b: Vec<char> = start.chars().collect();
            a.sort_unstable();
            b.sort_unstable();
            assert_eq!(a, b, "the search keeps the same 30 keys");
        }
    }

    #[test]
    fn the_left_out_key_costs_the_same_wherever_you_start() {
        let model = Model::new();
        // QWERTY leaves out the quote and Dvorak the slash: both cost an outer key.
        assert_eq!(
            metrics(&model, QWERTY, &Counts::of(&"'".repeat(5))).score,
            OFF_EFFORT
        );
        assert_eq!(
            metrics(&model, dvorak(), &Counts::of(&"/".repeat(5))).score,
            OFF_EFFORT
        );
        assert!(metrics(&model, QWERTY, &Counts::of(&"/".repeat(5))).score < OFF_EFFORT);
    }

    #[test]
    fn the_ladder_climbs_from_qwerty_to_the_target() {
        let target = "qwydgbhaujilstmpnoerzxcvk;f,./";
        let freq = std::array::from_fn(|i| 30 - i);
        let steps = ladder(QWERTY, target, &freq, 2);
        assert_eq!(steps.first().map(String::as_str), Some(QWERTY));
        assert_eq!(steps.last().map(String::as_str), Some(target));
        for w in steps.windows(2) {
            let changed = (0..30)
                .filter(|i| w[0].as_bytes()[*i] != w[1].as_bytes()[*i])
                .count();
            assert!(
                (2..=4).contains(&changed),
                "{changed} keys moved in one stage"
            );
        }
    }

    #[test]
    fn the_ladder_climbs_from_dvorak_too() {
        let freq = std::array::from_fn(|i| 31 - i);
        let model = Model::new();
        let c = Counts::of(&SAMPLE.repeat(20));
        let (_, p) = anneal(&model, &c, dvorak(), "", 5_000, &mut Rng::new(2));
        let target = keys_of(&p);
        let steps = ladder(dvorak(), &target, &freq, 2);
        assert_eq!(steps.first().map(String::as_str), Some(dvorak()));
        assert_eq!(steps.last(), Some(&target));
        let stages: String = steps
            .iter()
            .map(|k| format!("[[stage]]\nname = \"s\"\nkeys = {k:?}\n"))
            .collect();
        let words: String = (0..60).map(|i| format!("\"w{i}\", ")).collect();
        layout::Plan::parse(&format!("name = \"d\"\ncommon_words = [{words}]\n{stages}"))
            .expect("a plan from Dvorak loads");
    }

    #[test]
    fn tokens_keep_contractions() {
        assert_eq!(
            tokens("don't stop, it's 'fine'"),
            vec!["don't", "stop", "it's", "fine"]
        );
    }

    #[test]
    fn options_read_text_lists_and_flags() {
        let o =
            Options::parse(&["--text", "a.md", "notes", "--no-claude", "--name", "Hi"]).unwrap();
        assert_eq!(o.text, vec!["a.md", "notes"]);
        assert_eq!(
            Options::parse(&["--from", "dvorak"]).unwrap().from,
            "dvorak"
        );
        assert!(Options::parse(&["--from", "azerty-ish"]).is_err());
        assert!(!o.claude);
        assert_eq!(o.name, "Hi");
        assert!(Options::parse(&["--no-claude"]).is_err());
        assert!(Options::parse(&["--home-row-mods", "a,b"]).is_err());
    }

    #[test]
    fn share_links_carry_the_summary() {
        let url = share_url("# Home Row Hero\n\nqwe & more");
        assert!(url.starts_with(ISSUES));
        assert!(url.contains("title=Layout:%20Home%20Row%20Hero"));
        assert!(url.contains("qwe%20%26%20more"));
    }
}
