pub mod app;
pub mod charts;
pub mod coach;
pub mod corpus;
pub mod drill;
pub mod gate;
pub mod keymap;
pub mod layout;
pub mod optimise;
pub mod oryx;
pub mod plans;
pub mod report;
pub mod store;
pub mod warmup;

/// Opens a URL with `$VERY_NORMAL_OPEN` (a command, `{url}` marks where the
/// address goes), or with `open` (`xdg-open` on Linux).
pub fn open_url(url: &str) -> anyhow::Result<()> {
    let default = if cfg!(target_os = "linux") {
        "xdg-open {url}"
    } else {
        "open {url}"
    };
    let template = std::env::var("VERY_NORMAL_OPEN").unwrap_or_else(|_| default.to_string());
    let mut parts: Vec<String> = template.split_whitespace().map(str::to_string).collect();
    if !parts.iter().any(|p| p.contains("{url}")) {
        parts.push("{url}".into());
    }
    let parts: Vec<String> = parts.into_iter().map(|p| p.replace("{url}", url)).collect();
    let (cmd, args) = parts
        .split_first()
        .ok_or_else(|| anyhow::anyhow!("VERY_NORMAL_OPEN is empty"))?;
    std::process::Command::new(cmd)
        .args(args)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()?;
    Ok(())
}
