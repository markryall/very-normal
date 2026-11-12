//! Well-known layouts: where you might be starting from, and what yours is
//! compared with.
//!
//! A layout is the 30 main keys, left to right, top row first. They hold the
//! 26 letters and four of `; , . / '`. QWERTY's family keeps `'` on the outer
//! key beside the home row; Dvorak brings it in and sends `/` out.

use anyhow::{Result, bail};

/// Every character a layout's 30 keys can hold.
pub const UNIVERSE: &str = "abcdefghijklmnopqrstuvwxyz;,./'";

pub const QWERTY: &str = "qwertyuiopasdfghjkl;zxcvbnm,./";

/// Layouts people type on, with the names they go by.
pub const KNOWN: [(&str, &str); 7] = [
    ("QWERTY", QWERTY),
    ("Dvorak", "',.pyfgcrlaoeuidhtns;qjkxbmwvz"),
    ("Colemak", "qwfpgjluy;arstdhneiozxcvbkm,./"),
    ("Colemak-DH", "qwfpbjluy;arstgmneiozxcdvkh,./"),
    ("Workman", "qdrwbjfup;ashtgyneoizxmcvkl,./"),
    ("Norman", "qwdfkjurl;asetgyniohzxcvbpm,./"),
    ("Very Normal (Mark's)", "qwydgbhaujilstmpnoerzxcvk;f,./"),
];

/// The layouts people start from, offered first.
pub const STARTS: [&str; 6] = [
    "QWERTY",
    "Dvorak",
    "Colemak",
    "Colemak-DH",
    "Workman",
    "Norman",
];

fn simple(s: &str) -> String {
    s.chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

/// A known layout by name, ignoring case, spaces and dashes.
pub fn find(name: &str) -> Option<(&'static str, &'static str)> {
    let want = simple(name);
    KNOWN
        .iter()
        .find(|(n, _)| simple(n) == want || simple(n).starts_with(&want) && want.len() >= 4)
        .copied()
}

/// Where you're starting from: a known layout's name, or your 30 keys
/// written left to right, top row first (spaces are ignored).
pub fn start(s: &str) -> Result<(String, String)> {
    if let Some((name, keys)) = find(s) {
        return Ok((name.to_string(), keys.to_string()));
    }
    let keys: String = s
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect::<String>()
        .to_lowercase();
    if keys.chars().count() != 30 {
        bail!(
            "{s:?} isn't a layout I know ({}) or 30 keys written top row first",
            STARTS.join(", ")
        );
    }
    check(&keys)?;
    Ok(("your current layout".to_string(), keys))
}

/// 30 different keys from `UNIVERSE`, with every letter among them.
pub fn check(keys: &str) -> Result<()> {
    let mut seen = String::new();
    for c in keys.chars() {
        if !UNIVERSE.contains(c) {
            bail!("{c:?} can't be one of the 30 keys; they hold letters and ; , . / '");
        }
        if seen.contains(c) {
            bail!("{c:?} is on the layout twice");
        }
        seen.push(c);
    }
    if let Some(c) = ('a'..='z').find(|c| !seen.contains(*c)) {
        bail!("the layout has no {c}");
    }
    Ok(())
}

/// The one character of `UNIVERSE` a layout's 30 keys leave out.
pub fn left_out(keys: &str) -> char {
    UNIVERSE
        .chars()
        .find(|c| !keys.contains(*c))
        .unwrap_or('\'')
}

/// Keys that keep their places unless you say otherwise: Q, W, Z, X, C and V
/// for Cmd+Q, W, Z, X, C and V, and the punctuation (but `;`, which is rare).
pub fn default_lock(keys: &str) -> String {
    let mut lock = String::from("qwzxcv");
    lock.extend(keys.chars().filter(|c| matches!(c, ',' | '.' | '/' | '\'')));
    lock
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_layouts_are_whole() {
        for (name, keys) in KNOWN {
            assert_eq!(keys.chars().count(), 30, "{name}");
            check(keys).unwrap_or_else(|e| panic!("{name}: {e}"));
        }
    }

    #[test]
    fn finds_layouts_loosely() {
        assert_eq!(find("dvorak").map(|k| k.0), Some("Dvorak"));
        assert_eq!(find("colemak dh").map(|k| k.0), Some("Colemak-DH"));
        assert_eq!(find("colemak").map(|k| k.0), Some("Colemak"));
        assert_eq!(find("qwerty").map(|k| k.0), Some("QWERTY"));
        assert_eq!(find("nope"), None);
    }

    #[test]
    fn takes_your_own_keys() {
        let (name, keys) = start("qwerty uiop asdfghjkl; zxcvbnm,./").unwrap();
        assert_eq!(name, "your current layout");
        assert_eq!(keys, QWERTY);
        assert!(start("qwertyuiop").is_err());
        assert!(start("qwertyuiopasdfghjkl;zxcvbnm,.q").is_err());
    }

    #[test]
    fn locks_shortcuts_and_punctuation() {
        assert_eq!(default_lock(QWERTY), "qwzxcv,./");
        assert_eq!(default_lock(find("dvorak").unwrap().1), "qwzxcv',.");
        assert_eq!(left_out(QWERTY), '\'');
        assert_eq!(left_out(find("dvorak").unwrap().1), '/');
    }
}
