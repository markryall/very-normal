//! The ladder from QWERTY to Very Normal: one 30-key string per stage.
//!
//! Positions run left to right, top row then home row then bottom row, ten
//! keys a row (five per hand). Only letters and `; , . /` live here; the
//! quote key and the outer columns never move.

pub const STAGES: [&str; 10] = [
    "qwertyuiopasdfghjkl;zxcvbnm,./",
    "qwlrfyuiopasdtghjke;zxcvbnm,./",
    "qwlrfyuakpisdtghjoe;zxcvbnm,./",
    "qwlrfyuakpidstghnoe;zxcvbjm,./",
    "qwl;fyhakpidstgunoerzxcvbjm,./",
    "qw;dfyhakpilstgunoerzxcvbjm,./",
    "qw;dfyhaukilstgpnoerzxcvbjm,./",
    "qw;dgyhaukilstmpnoerzxcvbjf,./",
    "qwydgbhaukilstmpnoerzxcv;jf,./",
    "qwydgbhaujilstmpnoerzxcvk;f,./",
];

pub const LAST: usize = STAGES.len() - 1;

pub const NAMES: [&str; 10] = [
    "Baseline",
    "E and T come home",
    "I and O come home",
    "S and N come home",
    "R comes home",
    "L and D settle",
    "U and P settle",
    "G and M settle",
    "Y and B settle",
    "Fully Very Normal",
];

pub const BLURBS: [&str; 10] = [
    "Your layout as it is today. Record three 60-second real-text runs so later stages have numbers to beat.",
    "The two most common letters move onto the home row. Same-finger pairs drop by almost half straight away.",
    "I and O come home. A moves up to the top row, where it stays.",
    "S and N come home. 61% of your typing now lands on the home row.",
    "R takes the right pinky home key, and H and U trade places.",
    "L and D take their final spots. ; keeps wandering until the last stage.",
    "U and P settle. Same-finger pairs fall to 1.7%.",
    "G and M settle on the inner columns.",
    "Y and B settle on the top row.",
    "The last keys land. You're on the Very Normal layout.",
];

/// Share of your typing on the home row, and same-finger pairs, per stage.
pub const STATS: [(u8, f32); 10] = [
    (33, 6.5),
    (49, 3.9),
    (55, 4.1),
    (61, 3.9),
    (64, 3.0),
    (64, 2.7),
    (64, 1.7),
    (65, 1.4),
    (65, 1.2),
    (65, 1.0),
];

/// Vim commands that use the keys a stage moves.
pub const VIM: [&[&str]; 10] = [
    &[],
    &["e", "f", "t", "l"],
    &["a", "i", "o", "k"],
    &["dd", "j", "n", "s"],
    &["h", "r", "u"],
    &["dw", "l", ";"],
    &["k", "p", "u"],
    &["f", "gg", "m"],
    &["b", "yy", ";"],
    &["j", "k", ";"],
];

/// Home row mods: position and what the key does when held.
pub const MODS: [(usize, &str); 6] = [
    (11, "ctrl"),
    (12, "alt"),
    (13, "cmd"),
    (16, "cmd"),
    (17, "alt"),
    (18, "ctrl"),
];

pub const OUTER_LEFT: [&str; 3] = ["tab", "esc", "shf"];
pub const OUTER_RIGHT: [&str; 3] = ["bsp", "'", "ent"];

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

pub fn keys(stage: usize) -> Vec<char> {
    STAGES[stage].chars().collect()
}

/// The keys that change when you climb from `stage - 1` to `stage`.
pub fn changes(stage: usize) -> Vec<Change> {
    if stage == 0 || stage > LAST {
        return Vec::new();
    }
    let before = keys(stage - 1);
    keys(stage)
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
pub fn focus(stage: usize) -> Vec<char> {
    let mut out = Vec::new();
    for c in changes(stage) {
        if !out.contains(&c.to) {
            out.push(c.to);
        }
    }
    out
}

/// Whether a position types something different from QWERTY at this stage.
pub fn moved(stage: usize, index: usize) -> bool {
    keys(stage)[index] != keys(0)[index]
}

pub fn changed_now(stage: usize, index: usize) -> bool {
    changes(stage).iter().any(|c| c.index == index)
}

pub fn modifier(index: usize) -> Option<&'static str> {
    MODS.iter().find(|(i, _)| *i == index).map(|(_, m)| *m)
}

pub fn position(stage: usize, ch: char) -> Option<usize> {
    keys(stage).iter().position(|c| *c == ch)
}

/// The left-hand position that mirrors a right-hand one, and the reverse.
pub fn mirror(index: usize) -> usize {
    let (row, col) = (index / 10, index % 10);
    row * 10 + (9 - col)
}

/// Which stage a main layer's 30 letter keys match, if any.
pub fn find_stage(main: &str) -> Option<usize> {
    STAGES.iter().position(|s| *s == main)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_stage_is_a_rearrangement_of_qwerty() {
        let mut qwerty = keys(0);
        qwerty.sort();
        for stage in 0..=LAST {
            let mut k = keys(stage);
            k.sort();
            assert_eq!(k, qwerty, "stage {stage}");
        }
    }

    #[test]
    fn the_last_stage_is_very_normal() {
        assert_eq!(STAGES[LAST], "qwydgbhaujilstmpnoerzxcvk;f,./");
    }

    #[test]
    fn every_stage_moves_a_few_keys_and_keeps_the_shortcuts() {
        for stage in 1..=LAST {
            let n = changes(stage).len();
            assert!((3..=5).contains(&n), "stage {stage} moves {n}");
            for c in "qwzxcv".chars() {
                assert_eq!(
                    position(stage, c),
                    position(0, c),
                    "{c} moved in stage {stage}"
                );
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
        for (i, s) in STAGES.iter().enumerate() {
            assert_eq!(find_stage(s), Some(i));
        }
        assert_eq!(find_stage("nope"), None);
    }
}
