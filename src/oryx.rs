//! Reading a layout from Oryx, ZSA's configurator, to check a stage landed.
//!
//! Oryx has no documented API; this uses the GraphQL endpoint its own page
//! loads layouts from. It only works for layouts with a public link.

use std::process::Command;

use anyhow::{Context, Result, anyhow, bail};
use serde_json::Value;

use crate::layout;

pub const DEFAULT_LAYOUT: &str = "Xx6pM";

/// `$VERY_NORMAL_LAYOUT`, or Mark's voyager-very-normal layout.
pub fn layout_id() -> String {
    std::env::var("VERY_NORMAL_LAYOUT").unwrap_or_else(|_| DEFAULT_LAYOUT.to_string())
}

pub fn edit_url(layout: &str) -> String {
    format!("https://configure.zsa.io/voyager/layouts/{layout}/latest/0")
}

/// Where each of the 30 letter positions sits in Oryx's key list for the
/// Voyager: the left half's rows of six (outer column first), then the
/// right half's rows of six (outer column last).
const SLOTS: [usize; 30] = [
    1, 2, 3, 4, 5, 26, 27, 28, 29, 30, //
    7, 8, 9, 10, 11, 32, 33, 34, 35, 36, //
    13, 14, 15, 16, 17, 38, 39, 40, 41, 42,
];

const HOLDS: [(usize, &str); 6] = [
    (11, "LEFT_CTRL"),
    (12, "LEFT_ALT"),
    (13, "LEFT_GUI"),
    (16, "RIGHT_GUI"),
    (17, "RIGHT_ALT"),
    (18, "RIGHT_CTRL"),
];

#[derive(Debug, Clone, PartialEq)]
pub struct Snapshot {
    pub title: String,
    pub revision: String,
    pub created: String,
    /// The main layer's 30 letter keys; `?` where a key isn't a plain character.
    pub main: Vec<char>,
    /// What the six home row mod keys do when held.
    pub holds: Vec<(usize, String)>,
    /// The mirror layer's 15 left-hand letter keys, if there is a mirror layer.
    pub mirror: Option<Vec<char>>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Finding {
    Key {
        index: usize,
        got: char,
        want: char,
    },
    Hold {
        index: usize,
        got: String,
        want: &'static str,
    },
    Mirror {
        index: usize,
        got: char,
        want: char,
    },
}

impl std::fmt::Display for Finding {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Finding::Key { index, got, want } => write!(
                f,
                "main layer: the key where QWERTY has {} types {got}, should type {want}",
                layout::keys(0)[*index]
            ),
            Finding::Hold { index, got, want } => write!(
                f,
                "home row mod: the key where QWERTY has {} holds {}, should hold {want}",
                layout::keys(0)[*index],
                if got.is_empty() { "nothing" } else { got }
            ),
            Finding::Mirror { index, got, want } => write!(
                f,
                "mirror layer: the left key where QWERTY has {} types {got}, should type {want}",
                layout::keys(0)[*index]
            ),
        }
    }
}

pub fn fetch(layout: &str) -> Result<Snapshot> {
    let query = format!(
        "query {{ layout(hashId: \"{layout}\", geometry: \"voyager\", revisionId: \"latest\") \
         {{ title revision {{ hashId createdAt layers {{ title position keys }} }} }} }}"
    );
    let body = serde_json::json!({ "query": query }).to_string();
    let out = Command::new("curl")
        .args(["-sS", "--max-time", "20", "https://oryx.zsa.io/graphql"])
        .args(["-H", "Content-Type: application/json", "-d", &body])
        .output()
        .context("running curl")?;
    if !out.status.success() {
        bail!(
            "curl failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    let v: Value =
        serde_json::from_slice(&out.stdout).context("Oryx sent something that isn't JSON")?;
    parse(&v)
}

fn code(key: &Value, part: &str) -> String {
    key.get(part)
        .and_then(|p| p.get("code"))
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim_start_matches("KC_")
        .to_string()
}

fn character(key: &Value) -> char {
    let c = code(key, "tap");
    match c.as_str() {
        "SCLN" => ';',
        "COMMA" => ',',
        "DOT" => '.',
        "SLASH" => '/',
        "QUOTE" => '\'',
        s if s.len() == 1 => s.chars().next().unwrap().to_ascii_lowercase(),
        _ => '?',
    }
}

pub fn parse(v: &Value) -> Result<Snapshot> {
    let layout = v
        .pointer("/data/layout")
        .filter(|l| !l.is_null())
        .ok_or_else(|| anyhow!("Oryx didn't return that layout. Is its link public?"))?;
    let layers = layout
        .pointer("/revision/layers")
        .and_then(Value::as_array)
        .ok_or_else(|| anyhow!("the layout has no layers"))?;
    let keys_of = |layer: &Value| {
        layer
            .get("keys")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default()
    };
    let main = layers
        .iter()
        .find(|l| l.get("position").and_then(Value::as_u64) == Some(0))
        .map(keys_of)
        .ok_or_else(|| anyhow!("the layout has no main layer"))?;
    if main.len() < 43 {
        bail!(
            "the main layer has {} keys; is this a Voyager layout?",
            main.len()
        );
    }
    let mirror = layers
        .iter()
        .find(|l| {
            l.get("title")
                .and_then(Value::as_str)
                .is_some_and(|t| t.eq_ignore_ascii_case("mirror"))
        })
        .map(keys_of)
        .filter(|k| k.len() >= 43)
        .map(|k| {
            (0..3)
                .flat_map(|row| (0..5).map(move |col| row * 10 + col))
                .map(|i| character(&k[SLOTS[i]]))
                .collect()
        });
    let text = |p: &str| {
        layout
            .pointer(p)
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string()
    };
    Ok(Snapshot {
        title: text("/title"),
        revision: text("/revision/hashId"),
        created: text("/revision/createdAt"),
        main: SLOTS.iter().map(|s| character(&main[*s])).collect(),
        holds: HOLDS
            .iter()
            .map(|(i, _)| (*i, code(&main[SLOTS[*i]], "hold")))
            .collect(),
        mirror,
    })
}

impl Snapshot {
    pub fn main_string(&self) -> String {
        self.main.iter().collect()
    }

    /// The stage the main layer matches, if it matches one exactly.
    pub fn stage(&self) -> Option<usize> {
        layout::find_stage(&self.main_string())
    }

    /// Everything that differs from the given stage.
    pub fn compare(&self, stage: usize) -> Vec<Finding> {
        let want = layout::keys(stage);
        let mut out: Vec<Finding> = (0..30)
            .filter(|i| self.main[*i] != want[*i])
            .map(|index| Finding::Key {
                index,
                got: self.main[index],
                want: want[index],
            })
            .collect();
        for ((index, got), (_, want)) in self.holds.iter().zip(HOLDS) {
            if got != want {
                out.push(Finding::Hold {
                    index: *index,
                    got: got.clone(),
                    want,
                });
            }
        }
        if let Some(mirror) = &self.mirror {
            for (n, got) in mirror.iter().enumerate() {
                let index = (n / 5) * 10 + n % 5;
                let want = want[layout::mirror(index)];
                if *got != want {
                    out.push(Finding::Mirror {
                        index,
                        got: *got,
                        want,
                    });
                }
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(name: &str) -> Snapshot {
        let path = format!("{}/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"));
        let v: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
        parse(&v).unwrap()
    }

    #[test]
    fn reads_the_planck_layout_as_stage_zero() {
        let s = fixture("oryx-stage0.json");
        assert_eq!(s.title, "voyager-planck");
        assert_eq!(s.stage(), Some(0));
        assert!(s.compare(0).is_empty());
    }

    #[test]
    fn reads_the_very_normal_layout_as_stage_one() {
        let s = fixture("oryx-stage1.json");
        assert_eq!(s.stage(), Some(1));
        assert!(s.compare(1).is_empty(), "{:?}", s.compare(1));
    }

    #[test]
    fn lists_what_stage_two_still_needs() {
        let s = fixture("oryx-stage1.json");
        let f = s.compare(2);
        let keys = f
            .iter()
            .filter(|f| matches!(f, Finding::Key { .. }))
            .count();
        let mirror = f
            .iter()
            .filter(|f| matches!(f, Finding::Mirror { .. }))
            .count();
        assert_eq!(keys, layout::changes(2).len());
        assert_eq!(
            mirror,
            layout::changes(2).iter().filter(|c| c.right_hand()).count()
        );
    }

    #[test]
    fn a_missing_layout_is_explained() {
        let v = serde_json::json!({"data": {"layout": null}});
        assert!(parse(&v).unwrap_err().to_string().contains("public"));
    }
}
