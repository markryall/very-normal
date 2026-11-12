//! The plans you've made, and switching between them. Making a plan never
//! touches the one you're practising; `use` switches, and keeps each plan's
//! progress separately so stage numbers never get mixed up between plans.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};

use crate::layout::{self, Plan};

/// A file name for a plan's name: `Home Row Hero` is `home-row-hero`.
pub fn slug(name: &str) -> String {
    let mut out = String::new();
    for c in name.chars().flat_map(char::to_lowercase) {
        if c.is_alphanumeric() {
            out.push(c);
        } else if !out.is_empty() && !out.ends_with('-') {
            out.push('-');
        }
    }
    let out = out.trim_end_matches('-').to_string();
    if out.is_empty() { "plan".into() } else { out }
}

/// Where made plans and their summaries live: next to the active plan.
pub fn dir() -> PathBuf {
    layout::plan_path().with_file_name("plans")
}

pub fn made_path(name: &str) -> PathBuf {
    dir().join(format!("{}.toml", slug(name)))
}

/// A plan's shareable summary sits next to it.
pub fn summary_of(plan: &Path) -> PathBuf {
    plan.with_extension("md")
}

/// Where a plan's progress waits while you practise another one.
pub fn parked(progress: &Path, slug: &str) -> PathBuf {
    progress
        .with_file_name("plans")
        .join(format!("{slug}.json"))
}

/// Nothing to lose yet: no plan of your own and no practice recorded.
pub fn fresh(progress: &Path) -> bool {
    !layout::plan_path().exists() && !progress.exists()
}

/// The plan you're practising.
pub fn active_slug() -> String {
    slug(&layout::plan().name)
}

fn builtin_slug() -> String {
    slug(
        &Plan::parse(layout::EXAMPLE)
            .expect("the example plan is valid")
            .name,
    )
}

/// The plans you've made, by slug, plus the built-in one.
pub fn list() -> Vec<String> {
    let mut out: Vec<String> = std::fs::read_dir(dir())
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| {
            let p = e.path();
            (p.extension()? == "toml").then(|| p.file_stem()?.to_str().map(String::from))?
        })
        .collect();
    let builtin = builtin_slug();
    if !out.contains(&builtin) {
        out.push(builtin);
    }
    out.sort();
    out
}

/// Makes a plan the active one. Only for a fresh start: nothing to park.
pub fn activate(made: &Path) -> Result<()> {
    let to = layout::plan_path();
    if let Some(d) = to.parent() {
        std::fs::create_dir_all(d)?;
    }
    std::fs::copy(made, &to).with_context(|| format!("writing {}", to.display()))?;
    Ok(())
}

/// Switches to another plan: parks this plan's progress, and brings back
/// the other plan's progress if you've practised it before.
pub fn switch(target: &str, progress: &Path) -> Result<String> {
    let current = active_slug();
    let want = slug(target);
    if want == current {
        bail!("you're already practising {}", layout::plan().name);
    }
    let made = dir().join(format!("{want}.toml"));
    let builtin = want == builtin_slug() && !made.exists();
    if !builtin && !made.exists() {
        bail!("no plan called {target}; you have: {}", list().join(", "));
    }
    let plan = if builtin {
        Plan::parse(layout::EXAMPLE)?
    } else {
        Plan::load(&made)?
    };

    // Keep a plan you wrote by hand, which has no copy in plans/ yet.
    let active = layout::plan_path();
    let keep = dir().join(format!("{current}.toml"));
    if active.exists() && !keep.exists() {
        std::fs::create_dir_all(dir())?;
        std::fs::copy(&active, &keep).with_context(|| format!("keeping {}", keep.display()))?;
    }

    if progress.exists() {
        let to = parked(progress, &current);
        std::fs::create_dir_all(to.parent().expect("has a parent"))?;
        std::fs::rename(progress, &to).with_context(|| format!("parking {}", to.display()))?;
    }
    let back = parked(progress, &want);
    let resumed = back.exists();
    if resumed {
        std::fs::rename(&back, progress)
            .with_context(|| format!("restoring {}", back.display()))?;
    }

    if builtin {
        if active.exists() {
            std::fs::remove_file(&active)?;
        }
    } else {
        activate(&made)?;
    }
    Ok(format!(
        "Now practising {}{}. Your progress on {current} is kept for when you switch back.",
        plan.name,
        if resumed {
            ", where you left off"
        } else {
            " from the start"
        }
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slugs_are_file_names() {
        assert_eq!(slug("Home Row Hero"), "home-row-hero");
        assert_eq!(slug("  Very  Normal! "), "very-normal");
        assert_eq!(slug("Mark's"), "mark-s");
        assert_eq!(slug("!!"), "plan");
    }
}
