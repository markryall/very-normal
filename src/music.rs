//! Gentle music in the background during a guided session: an ambient
//! Icecast stream played by mpv or ffplay, if either is installed.

use std::process::{Child, Command, Stdio};

/// SomaFM Drone Zone: slow, ambient, wordless.
pub const DEFAULT_URL: &str = "https://ice1.somafm.com/dronezone-128-mp3";

/// The stream to play: `$VERY_NORMAL_MUSIC`, or Drone Zone.
pub fn url() -> String {
    url_from(std::env::var("VERY_NORMAL_MUSIC").ok())
}

fn url_from(var: Option<String>) -> String {
    var.filter(|v| !v.trim().is_empty())
        .unwrap_or_else(|| DEFAULT_URL.to_string())
}

/// The command that plays `url`, given which programs are on the PATH.
pub fn player_command(url: &str, available: impl Fn(&str) -> bool) -> Option<Vec<String>> {
    let parts: &[&str] = if available("mpv") {
        &["mpv", "--no-video", "--really-quiet"]
    } else if available("ffplay") {
        &["ffplay", "-nodisp", "-loglevel", "quiet"]
    } else {
        return None;
    };
    let mut cmd: Vec<String> = parts.iter().map(|s| s.to_string()).collect();
    cmd.push(url.to_string());
    Some(cmd)
}

/// Whether `program` is an executable somewhere on the PATH.
pub fn on_path(program: &str) -> bool {
    std::env::var_os("PATH")
        .is_some_and(|paths| std::env::split_paths(&paths).any(|dir| dir.join(program).is_file()))
}

/// The player while it runs. Dropping it stops the music.
#[derive(Default)]
pub struct Music {
    child: Option<Child>,
}

impl Music {
    pub fn playing(&self) -> bool {
        self.child.is_some()
    }

    /// Starts the stream, or says why it can't.
    pub fn start(&mut self) -> Result<(), String> {
        let missing = "No player for the music: `brew install mpv`, or set VERY_NORMAL_MUSIC. Any gentle music you like works just as well.";
        let cmd = player_command(&url(), on_path).ok_or_else(|| missing.to_string())?;
        let child = Command::new(&cmd[0])
            .args(&cmd[1..])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("Couldn't start {}: {e}", cmd[0]))?;
        self.child = Some(child);
        Ok(())
    }

    pub fn stop(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }

    /// True once, when the player has exited by itself (no network, say).
    pub fn ended(&mut self) -> bool {
        let exited = self
            .child
            .as_mut()
            .is_some_and(|c| matches!(c.try_wait(), Ok(Some(_))));
        if exited {
            self.child = None;
        }
        exited
    }
}

impl Drop for Music {
    fn drop(&mut self) {
        self.stop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plays_drone_zone_unless_told_otherwise() {
        assert_eq!(url_from(None), DEFAULT_URL);
        assert_eq!(url_from(Some(String::new())), DEFAULT_URL);
        assert_eq!(url_from(Some("http://x/y".into())), "http://x/y");
    }

    #[test]
    fn prefers_mpv_then_ffplay() {
        let cmd = player_command("u", |_| true).unwrap();
        assert_eq!(cmd, ["mpv", "--no-video", "--really-quiet", "u"]);
        let cmd = player_command("u", |p| p == "ffplay").unwrap();
        assert_eq!(cmd, ["ffplay", "-nodisp", "-loglevel", "quiet", "u"]);
        assert_eq!(player_command("u", |_| false), None);
    }

    #[test]
    fn notices_a_player_that_stopped_by_itself() {
        let mut m = Music {
            child: Some(Command::new("true").spawn().unwrap()),
        };
        std::thread::sleep(std::time::Duration::from_millis(200));
        assert!(m.ended());
        assert!(!m.playing());
        assert!(!m.ended());
    }
}
