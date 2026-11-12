pub mod app;
pub mod coach;
pub mod drill;
pub mod gate;
pub mod layout;
pub mod oryx;
pub mod report;
pub mod store;

/// Opens a URL with `$VERY_NORMAL_OPEN` (a command, `{url}` marks where the
/// address goes), or with macOS `open`.
pub fn open_url(url: &str) -> anyhow::Result<()> {
    let template = std::env::var("VERY_NORMAL_OPEN").unwrap_or_else(|_| "open {url}".to_string());
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
