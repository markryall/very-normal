//! Drill vocabulary: words Mark actually types, picked out of his own
//! messages and filtered against a dictionary.

use std::collections::HashMap;
use std::sync::OnceLock;

use serde::Deserialize;

use crate::layout::LAST;

#[derive(Deserialize)]
struct Raw {
    common: Vec<String>,
    #[serde(flatten)]
    stages: HashMap<String, Vec<String>>,
}

pub struct Words {
    /// Most common first.
    pub common: Vec<String>,
    /// Words heavy in each stage's moved keys, indexed by stage (0 is empty).
    pub stages: Vec<Vec<String>>,
}

pub fn get() -> &'static Words {
    static WORDS: OnceLock<Words> = OnceLock::new();
    WORDS.get_or_init(|| {
        let raw: Raw = serde_json::from_str(include_str!("../data/words.json"))
            .expect("data/words.json is valid");
        let stages = (0..=LAST)
            .map(|s| raw.stages.get(&s.to_string()).cloned().unwrap_or_default())
            .collect();
        Words {
            common: raw.common,
            stages,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_stage_has_words_with_its_moved_keys() {
        let w = get();
        assert!(w.common.len() >= 300);
        for stage in 1..=LAST {
            let focus = crate::layout::focus(stage);
            let list = &w.stages[stage];
            assert!(list.len() >= 30, "stage {stage}");
            assert!(
                list.iter()
                    .all(|word| word.chars().any(|c| focus.contains(&c))),
                "stage {stage} has a word without its keys"
            );
        }
    }
}
