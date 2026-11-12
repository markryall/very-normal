//! What you typed: the messages you sent Claude Code, and any other writing
//! you point at. Nothing here leaves your machine.

use std::io::Read;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde_json::Value;

/// Messages longer than this were probably pasted rather than typed.
const LONGEST: usize = 2000;

/// File types `--text` reads when it's given a folder.
const TEXT_EXTENSIONS: [&str; 8] = ["txt", "md", "markdown", "org", "rst", "text", "tex", "eml"];

/// Where Claude Code keeps its transcripts.
pub fn claude_projects() -> PathBuf {
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_default();
    home.join(".claude/projects")
}

/// The messages you typed to Claude Code: user turns with plain text, minus
/// pastes, slash commands, interruptions and anything long enough to be a
/// paste. Never Claude's replies or tool output.
pub fn typed_messages(projects: &Path) -> Vec<String> {
    let mut files = Vec::new();
    walk(projects, &mut files, &|p| {
        p.extension().is_some_and(|e| e == "jsonl")
    });
    files.retain(|p| !p.to_string_lossy().contains("/subagents/"));
    files.sort();
    let mut out = Vec::new();
    for file in files {
        let Ok(bytes) = std::fs::read(&file) else {
            continue;
        };
        for line in String::from_utf8_lossy(&bytes).lines() {
            if let Some(m) = typed_message(line) {
                out.push(m);
            }
        }
    }
    out
}

fn typed_message(line: &str) -> Option<String> {
    let o: Value = serde_json::from_str(line).ok()?;
    let truthy = |k: &str| {
        o.get(k)
            .is_some_and(|v| v.as_bool().unwrap_or(!v.is_null()))
    };
    if o.get("type").and_then(Value::as_str) != Some("user")
        || truthy("isMeta")
        || truthy("isSidechain")
    {
        return None;
    }
    let content = o.pointer("/message/content")?;
    let text = match content {
        Value::String(s) => s.clone(),
        Value::Array(parts) => parts
            .iter()
            .filter(|p| p.get("type").and_then(Value::as_str) == Some("text"))
            .map(|p| p.get("text").and_then(Value::as_str).unwrap_or(""))
            .collect::<Vec<_>>()
            .join(" "),
        _ => return None,
    };
    let text = without_pastes(&text);
    let text = text.trim();
    let head: String = text.chars().take(20).collect();
    if text.is_empty()
        || text.starts_with('<')
        || text.starts_with("[Request interrupted")
        || head.contains("Caveat:")
        || text.chars().count() > LONGEST
    {
        return None;
    }
    Some(text.to_string())
}

/// Drops `<pasted_content …>…</pasted_content …>` blocks.
fn without_pastes(text: &str) -> String {
    let mut out = String::new();
    let mut rest = text;
    while let Some(start) = rest.find("<pasted_content") {
        let Some(close) = rest[start..].find("</pasted_content") else {
            break;
        };
        let after_close = start + close;
        let Some(end) = rest[after_close..].find('>') else {
            break;
        };
        out.push_str(&rest[..start]);
        rest = &rest[after_close + end + 1..];
    }
    out.push_str(rest);
    out
}

/// Paragraphs from text files, folders of them (their .txt, .md, .org and
/// similar files), or `-` for stdin.
pub fn text_messages(paths: &[String]) -> Result<Vec<String>> {
    let mut out = Vec::new();
    for path in paths {
        if path == "-" {
            let mut body = String::new();
            std::io::stdin().read_to_string(&mut body)?;
            out.extend(paragraphs(&body));
            continue;
        }
        let p = Path::new(path);
        let mut files = Vec::new();
        if p.is_dir() {
            walk(p, &mut files, &|f| {
                f.extension()
                    .and_then(|e| e.to_str())
                    .is_some_and(|e| TEXT_EXTENSIONS.contains(&e.to_ascii_lowercase().as_str()))
            });
            files.sort();
        } else {
            files.push(p.to_path_buf());
        }
        for f in files {
            let bytes = std::fs::read(&f).with_context(|| format!("reading {}", f.display()))?;
            out.extend(paragraphs(&String::from_utf8_lossy(&bytes)));
        }
    }
    Ok(out)
}

/// Splits on blank lines.
fn paragraphs(body: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut current: Vec<&str> = Vec::new();
    for line in body.lines() {
        if line.trim().is_empty() {
            if !current.is_empty() {
                out.push(current.join("\n").trim().to_string());
                current.clear();
            }
        } else {
            current.push(line);
        }
    }
    if !current.is_empty() {
        out.push(current.join("\n").trim().to_string());
    }
    out.retain(|p| !p.is_empty());
    out
}

/// Every file under `dir` that `keep` wants, skipping hidden folders.
fn walk(dir: &Path, out: &mut Vec<PathBuf>, keep: &dyn Fn(&Path) -> bool) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let hidden = entry.file_name().to_string_lossy().starts_with('.');
        match entry.file_type() {
            Ok(t) if t.is_dir() && !hidden => walk(&path, out, keep),
            Ok(t) if t.is_file() && keep(&path) => out.push(path),
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_only_what_you_typed() {
        let msg = |content: &str| {
            format!(r#"{{"type":"user","message":{{"role":"user","content":{content}}}}}"#)
        };
        assert_eq!(
            typed_message(&msg(r#""make it faster""#)).as_deref(),
            Some("make it faster")
        );
        assert_eq!(
            typed_message(&msg(
                r#"[{"type":"text","text":"hi"},{"type":"tool_result","content":"x"}]"#
            ))
            .as_deref(),
            Some("hi")
        );
        assert_eq!(
            typed_message(&msg(
                r#""look <pasted_content id=\"1\">secret</pasted_content id=\"1\"> here""#
            ))
            .as_deref(),
            Some("look  here")
        );
        assert_eq!(
            typed_message(&msg(r#""<command-name>/clear</command-name>""#)),
            None
        );
        assert_eq!(
            typed_message(&msg(r#""[Request interrupted by user]""#)),
            None
        );
        assert_eq!(
            typed_message(&msg(&format!("\"{}\"", "a".repeat(2001)))),
            None
        );
        assert_eq!(
            typed_message(r#"{"type":"assistant","message":{"content":"hello"}}"#),
            None
        );
        assert_eq!(
            typed_message(r#"{"type":"user","isMeta":true,"message":{"content":"hello"}}"#),
            None
        );
    }

    #[test]
    fn splits_text_into_paragraphs() {
        assert_eq!(
            paragraphs("one\ntwo\n\n  \nthree\n"),
            vec!["one\ntwo".to_string(), "three".to_string()]
        );
    }
}
