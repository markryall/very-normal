# Brief

Before practice, recommend gentle music on headphones and a quiet place where you won't be interrupted. Perhaps there's a suitable Icecast stream (or Creative Commons music) that the tutor could play in the background.

---

## Goal

The guided session's layout screen suggests a quiet place, headphones and gentle music, and offers a key that plays an ambient Icecast stream in the background through a local player until it's toggled off or the tutor quits.

## Acceptance criteria

1. The layout screen (card 02) shows one line suggesting a quiet place where you won't be interrupted, headphones, and gentle music, plus `m music` in its help line.
2. `music::url()` returns `$VERY_NORMAL_MUSIC` when it is set and non-empty, else the default `https://ice1.somafm.com/dronezone-128-mp3` (SomaFM Drone Zone, an ambient Icecast stream).
3. `music::player_command(url, available)` returns the command to run, where `available` says which programs are on PATH. It returns `mpv --no-video --really-quiet <url>` when mpv is available, else `ffplay -nodisp -loglevel quiet <url>` when ffplay is, else `None`.
4. `m` in `Phase::Layout`, `Phase::Warmup` or `Phase::Report` toggles the music. Starting it spawns the player with stdin, stdout and stderr set to null, so nothing writes over the TUI. Stopping it kills the child and waits for it.
5. While music is playing, the help line in those phases shows `m stop music`.
6. With no player available, `m` doesn't spawn anything and shows a message saying to install mpv (`brew install mpv`) or set `VERY_NORMAL_MUSIC`, and that any music you like works just as well.
7. The player is killed when the app quits, by `q`, Ctrl-C or an error return from the run loop (a `Drop` on the music handle).
8. If the player exits on its own (e.g. no network), the next tick notices, clears the playing state, and shows a message that the stream stopped.
9. `m` during typing types the letter as usual. It never toggles music in `Phase::Practice` while typing.
10. README documents the music key, the default stream, `VERY_NORMAL_MUSIC`, and that mpv or ffplay is needed.
11. Unit tests cover `music::url()` with and without the variable, and `player_command` for all three cases. No test spawns a real player or touches the network.
12. `cargo test` and `cargo clippy --all-targets -- -D warnings` pass.

## Out of scope

- Downloading or bundling music files.
- Volume control, a choice of stations in the UI, or now-playing metadata.
- Players other than mpv and ffplay (`afplay` can't play streams).
- Music on the dashboard.

## Open questions

None. Mark should listen to the default stream once to confirm it suits (the sandbox couldn't reach it while this was built).
