//! Software keymaps for a stage, for keyboards without Oryx: kanata (macOS,
//! Linux, Windows) and Karabiner-Elements (macOS).
//!
//! Both assume the keyboard itself still sends QWERTY. Don't use them on a
//! board whose firmware already has the stage, or the keys move twice.

use serde_json::json;

use crate::layout;

/// The physical key at each position, as the OS sees it.
const PHYSICAL: &str = "qwertyuiopasdfghjkl;zxcvbnm,./";

/// Positions where this stage types something other than the physical key.
fn remaps(stage: usize) -> Vec<(char, char)> {
    PHYSICAL
        .chars()
        .zip(layout::keys(stage))
        .filter(|(from, to)| from != to)
        .collect()
}

fn header(stage: usize) -> String {
    format!(
        "{}, stage {stage} of {}: {}",
        layout::plan().name,
        layout::last(),
        layout::name(stage)
    )
}

pub fn kanata(stage: usize) -> String {
    let spaced = |s: &str| s.chars().map(String::from).collect::<Vec<_>>().join(" ");
    let keys: String = layout::keys(stage).into_iter().collect();
    format!(
        ";; very-normal: {}\n\
         ;; The keyboard must send QWERTY; don't also remap it in firmware.\n\
         (defcfg process-unmapped-keys yes)\n\
         (defsrc {})\n\
         (deflayer stage{stage} {})\n",
        header(stage),
        spaced(PHYSICAL),
        spaced(&keys),
    )
}

fn key_code(c: char) -> String {
    match c {
        ';' => "semicolon".into(),
        ',' => "comma".into(),
        '.' => "period".into(),
        '/' => "slash".into(),
        '\'' => "quote".into(),
        c => c.to_string(),
    }
}

/// A complex modification for `~/.config/karabiner/assets/complex_modifications/`.
pub fn karabiner(stage: usize) -> String {
    let manipulators: Vec<_> = remaps(stage)
        .into_iter()
        .map(|(from, to)| {
            json!({
                "type": "basic",
                "from": { "key_code": key_code(from), "modifiers": { "optional": ["any"] } },
                "to": [{ "key_code": key_code(to) }],
            })
        })
        .collect();
    let doc = json!({
        "title": format!("very-normal: {}", layout::plan().name),
        "rules": [{
            "description": format!("{} (the keyboard must send QWERTY)", header(stage)),
            "manipulators": manipulators,
        }],
    });
    serde_json::to_string_pretty(&doc).expect("json") + "\n"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kanata_maps_every_key_to_the_stage() {
        let k = kanata(1);
        assert!(k.contains("(defsrc q w e r t y u i o p a s d f g h j k l ; z x c v b n m , . /)"));
        assert!(k.contains(
            "(deflayer stage1 q w l r f y u i o p a s d t g h j k e ; z x c v b n m , . /)"
        ));
    }

    #[test]
    fn karabiner_only_remaps_what_moved() {
        let v: serde_json::Value = serde_json::from_str(&karabiner(1)).unwrap();
        let m = v["rules"][0]["manipulators"].as_array().unwrap();
        assert_eq!(m.len(), 4);
        assert_eq!(m[0]["from"]["key_code"], "e");
        assert_eq!(m[0]["to"][0]["key_code"], "l");
    }

    #[test]
    fn karabiner_names_punctuation() {
        let v: serde_json::Value = serde_json::from_str(&karabiner(layout::last())).unwrap();
        let codes: Vec<&str> = v["rules"][0]["manipulators"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|m| m["to"][0]["key_code"].as_str())
            .collect();
        assert!(codes.contains(&"semicolon"));
        assert!(remaps(0).is_empty());
    }
}
