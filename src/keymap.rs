//! Software keymaps for a stage, for keyboards without Oryx: kanata (macOS,
//! Linux, Windows) and Karabiner-Elements (macOS).
//!
//! Both assume the keyboard itself sends QWERTY and the computer is set to
//! the U.S. (QWERTY) layout: they make the whole layout themselves, whatever
//! you started from. When a plan's 30 keys take in the quote (as Dvorak's
//! do), the quote key beside the home row types what they left out. A keyboard whose
//! firmware already has the stage must be left out, or its keys move twice:
//! the Karabiner rule only touches the built-in keyboard unless asked
//! otherwise, and the kanata config shows how to pick devices.

use serde_json::json;

use crate::{layout, named};

/// The physical key at each position, as the OS sees it.
const PHYSICAL: &str = "qwertyuiopasdfghjkl;zxcvbnm,./";

/// Every key this stage sets: the 30, and the quote key beside the home
/// row, which types whatever the 30 leave out.
fn keys(stage: usize) -> Vec<(char, char)> {
    let mut out: Vec<(char, char)> = PHYSICAL.chars().zip(layout::keys(stage)).collect();
    let stage_keys: String = layout::keys(stage).into_iter().collect();
    out.push(('\'', named::left_out(&stage_keys)));
    out
}

/// Keys where this stage types something other than the physical key.
fn remaps(stage: usize) -> Vec<(char, char)> {
    keys(stage)
        .into_iter()
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
    let all = keys(stage);
    let from: Vec<String> = all.iter().map(|(f, _)| f.to_string()).collect();
    let to: Vec<String> = all.iter().map(|(_, t)| t.to_string()).collect();
    format!(
        ";; very-normal: {}\n\
         ;; This remaps every keyboard kanata grabs. Leave out any keyboard whose\n\
         ;; firmware already has the stage (a remapped Voyager), or its keys move twice.\n\
         ;; On macOS, uncomment the line below to grab only the built-in keyboard.\n\
         ;; On Linux use linux-dev to list devices; on Windows, kanata's interception build.\n\
         ;; Set the computer to the U.S. (QWERTY) layout: this makes the whole layout itself.\n\
         (defcfg\n  \
           process-unmapped-keys yes\n  \
           ;; macos-dev-names-include (\"Apple Internal Keyboard / Trackpad\")\n\
         )\n\
         (defsrc {})\n\
         (deflayer stage{stage} {})\n",
        header(stage),
        from.join(" "),
        to.join(" "),
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
/// Unless `all_keyboards`, it only applies to the Mac's built-in keyboard, so
/// an external keyboard that already has the stage in firmware is left alone.
pub fn karabiner(stage: usize, all_keyboards: bool) -> String {
    let built_in = json!([{
        "type": "device_if",
        "identifiers": [{ "is_built_in_keyboard": true }],
    }]);
    let manipulators: Vec<_> = remaps(stage)
        .into_iter()
        .map(|(from, to)| {
            let mut m = json!({
                "type": "basic",
                "from": { "key_code": key_code(from), "modifiers": { "optional": ["any"] } },
                "to": [{ "key_code": key_code(to) }],
            });
            if !all_keyboards {
                m["conditions"] = built_in.clone();
            }
            m
        })
        .collect();
    let which = if all_keyboards {
        "every keyboard; each must send QWERTY"
    } else {
        "built-in keyboard only"
    };
    let doc = json!({
        "title": format!("very-normal: {}", layout::plan().name),
        "rules": [{
            "description": format!("{} ({which})", header(stage)),
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
        assert!(
            k.contains("(defsrc q w e r t y u i o p a s d f g h j k l ; z x c v b n m , . / ')")
        );
        assert!(k.contains(
            "(deflayer stage1 q w l r f y u i o p a s d t g h j k e ; z x c v b n m , . / ')"
        ));
    }

    #[test]
    fn karabiner_only_remaps_what_moved() {
        let v: serde_json::Value = serde_json::from_str(&karabiner(1, false)).unwrap();
        let m = v["rules"][0]["manipulators"].as_array().unwrap();
        assert_eq!(m.len(), 4);
        assert_eq!(
            m[0]["conditions"][0]["identifiers"][0]["is_built_in_keyboard"],
            true
        );
        assert_eq!(m[0]["from"]["key_code"], "e");
        assert_eq!(m[0]["to"][0]["key_code"], "l");
    }

    #[test]
    fn karabiner_names_punctuation() {
        let v: serde_json::Value = serde_json::from_str(&karabiner(layout::last(), true)).unwrap();
        assert!(v["rules"][0]["manipulators"][0].get("conditions").is_none());
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
