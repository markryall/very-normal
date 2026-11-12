//! A charts page: one self-contained HTML file built from your progress, with
//! nothing fetched from the network, so it opens anywhere and works offline.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde_json::json;

use crate::store::Progress;
use crate::{coach, gate, layout};

const TEMPLATE: &str = include_str!("charts.html");
const DATA_MARK: &str = "\"__VERY_NORMAL_DATA__\"";

/// Next to the progress file: `~/.local/share/very-normal/charts.html`.
pub fn default_path(progress: &Path) -> PathBuf {
    progress.with_file_name("charts.html")
}

/// The page, with your progress, the plan's stages and the coach's and gate's
/// numbers baked in.
pub fn html(progress: &Progress, now_ms: u64) -> String {
    let stages: Vec<_> = (0..=layout::last())
        .map(|s| {
            json!({
                "name": layout::name(s),
                "keys": layout::keys_string(s),
                "focus": layout::focus(s).iter().collect::<String>(),
            })
        })
        .collect();
    let data = json!({
        "generated": now_ms,
        "plan": layout::plan().name,
        "stages": stages,
        "baseline": progress.baseline(),
        "progress": progress,
        "goal": coach::DAILY_GOAL,
        "breakMinutes": coach::BREAK,
        "gate": {
            "wordsAccuracy": gate::WORDS_ACCURACY,
            "speedShare": gate::SPEED_SHARE,
            "realAccuracy": gate::REAL_ACCURACY,
        },
    });
    // `</` inside a script would end it early
    let data = data.to_string().replace("</", "<\\/");
    TEMPLATE.replacen(DATA_MARK, &data, 1)
}

pub fn write(progress: &Progress, path: &Path, now_ms: u64) -> Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
    }
    std::fs::write(path, html(progress, now_ms))
        .with_context(|| format!("writing {}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_data_lands_in_the_page_and_cannot_close_the_script() {
        let mut p = Progress::default();
        p.manual_mut(0).wpm = 33.0;
        p.warmed_up = Some("</script><b>".into());
        let page = html(&p, 1);
        assert!(!page.contains(DATA_MARK));
        assert!(page.contains("\"baseline\":33.0"));
        assert!(!page.contains("</script><b>"));
        assert_eq!(page.matches("</script>").count(), 1);
    }
}
