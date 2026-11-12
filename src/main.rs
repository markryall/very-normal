use std::io::Read;
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};
use ratatui::DefaultTerminal;
use ratatui::crossterm::event::{self, Event, KeyEventKind};
use very_normal::app::App;
use very_normal::layout;
use very_normal::store::{self, Progress};
use very_normal::{charts, coach, keymap, oryx, report};

const HELP: &str = "\
very-normal: walk a ZSA Voyager from QWERTY to the Very Normal layout

usage: very-normal                  the trainer (TUI)
       very-normal status           what the coach thinks of your practice today
       very-normal next             the Oryx edits for the next stage, and where to make them
       very-normal check            read the layout from Oryx and compare it with your stage
       very-normal stage <n>        set the stage you're practising (0-9)
       very-normal baseline <wpm>   type in a baseline instead of recording one
       very-normal import <file|->  merge a backup copied out of the web tutor
       very-normal plan             which plan you're on, and its stages
       very-normal keymap kanata|karabiner [stage] [--all-keyboards]
                                    a software keymap for keyboards without Oryx
                                    (karabiner: built-in keyboard only, unless --all-keyboards)
       very-normal charts [file] [--no-open]
                                    write your progress as a page of charts and open it
       very-normal export           print your progress as JSON
       very-normal --help | --version

Progress lives in ~/.local/share/very-normal/progress.json ($VERY_NORMAL_DATA).
The Oryx layout is Xx6pM ($VERY_NORMAL_LAYOUT); URLs open with `open`
($VERY_NORMAL_OPEN, where {url} marks the address).
";

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let plan_file = layout::install()?;
    let path = store::default_path();
    let words: Vec<&str> = args.iter().map(String::as_str).collect();
    match words.as_slice() {
        [] => {
            let progress = Progress::load(&path)?;
            let mut terminal = ratatui::init();
            let result = run(&mut terminal, App::new(progress, path, Instant::now()));
            ratatui::restore();
            result?;
        }
        ["-h" | "--help" | "help"] => print!("{HELP}"),
        ["-V" | "--version"] => println!("very-normal {}", env!("CARGO_PKG_VERSION")),
        ["next"] => {
            let p = Progress::load(&path)?;
            for line in report::next_stage(p.stage, &oryx::edit_url(&oryx::layout_id())) {
                println!("{line}");
            }
        }
        ["check"] => {
            let p = Progress::load(&path)?;
            let snap = oryx::fetch(&oryx::layout_id())?;
            println!("You're practising stage {}.", p.stage);
            for line in report::check(&snap, p.stage) {
                println!("{line}");
            }
        }
        ["stage", n] => {
            let n: usize = n.parse().context("the stage is a number")?;
            if n > layout::last() {
                bail!("there are only stages 0 to {}", layout::last());
            }
            let mut p = Progress::load(&path)?;
            p.stage = n;
            p.save(&path)?;
            println!("Now practising stage {n}.");
        }
        ["baseline", wpm] => {
            let wpm: f64 = wpm
                .parse()
                .context("the baseline is a number of words per minute")?;
            let mut p = Progress::load(&path)?;
            p.manual_mut(0).wpm = wpm;
            p.save(&path)?;
            println!("Baseline set to {wpm:.0} WPM.");
        }
        ["import", file] => {
            let text = if *file == "-" {
                let mut s = String::new();
                std::io::stdin().read_to_string(&mut s)?;
                s
            } else {
                std::fs::read_to_string(file).with_context(|| format!("reading {file}"))?
            };
            let incoming =
                Progress::parse(text.trim()).context("that isn't a backup from the tutor")?;
            let mut p = Progress::load(&path)?;
            let added = p.merge(incoming);
            p.save(&path)?;
            println!("Imported {added} new run(s). Practising stage {}.", p.stage);
        }
        ["status"] => {
            let p = Progress::load(&path)?;
            let a = coach::assess(&p.runs, now_ms()?);
            println!("{}", a.message);
            println!(
                "today {:.0}/{:.0} min · {}-day streak · practising stage {}",
                a.today_minutes,
                coach::DAILY_GOAL,
                a.streak,
                p.stage
            );
        }
        ["plan"] => {
            let plan = layout::plan();
            match &plan_file {
                Some(p) => println!("Plan: {} ({})", plan.name, p.display()),
                None => println!(
                    "Plan: {} (built in; make your own at {})",
                    plan.name,
                    layout::plan_path().display()
                ),
            }
            for s in 0..=layout::last() {
                println!("  {s}: {}  {}", layout::keys_string(s), layout::name(s));
            }
        }
        ["keymap", kind, rest @ ..] => {
            let all = rest.contains(&"--all-keyboards");
            let stage = match rest
                .iter()
                .filter(|a| **a != "--all-keyboards")
                .collect::<Vec<_>>()[..]
            {
                [] => Progress::load(&path)?.stage,
                [n] => n.parse().context("the stage is a number")?,
                _ => bail!("usage: very-normal keymap kanata|karabiner [stage] [--all-keyboards]"),
            };
            if stage > layout::last() {
                bail!("there are only stages 0 to {}", layout::last());
            }
            match *kind {
                "kanata" => print!("{}", keymap::kanata(stage)),
                "karabiner" => print!("{}", keymap::karabiner(stage, all)),
                other => bail!("no {other} keymap; try kanata or karabiner"),
            }
        }
        ["charts", rest @ ..] => {
            let open = !rest.contains(&"--no-open");
            let out = match rest
                .iter()
                .filter(|a| **a != "--no-open")
                .collect::<Vec<_>>()[..]
            {
                [] => charts::default_path(&path),
                [f] => std::path::PathBuf::from(f),
                _ => bail!("usage: very-normal charts [file] [--no-open]"),
            };
            let p = Progress::load(&path)?;
            charts::write(&p, &out, now_ms()?)?;
            println!("Charts written to {}", out.display());
            if open {
                very_normal::open_url(&out.display().to_string())?;
            }
        }
        ["export"] => println!("{}", serde_json::to_string_pretty(&Progress::load(&path)?)?),
        _ => {
            eprint!("{HELP}");
            bail!("didn't understand: {}", args.join(" "));
        }
    }
    Ok(())
}

fn now_ms() -> Result<u64> {
    Ok(std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_millis() as u64)
}

/// Runs the TUI until it quits, redrawing often so timers and hints move.
fn run(terminal: &mut DefaultTerminal, mut app: App) -> Result<()> {
    while !app.should_quit {
        let now = Instant::now();
        app.tick(now);
        terminal.draw(|frame| app.render(frame, now))?;
        if event::poll(Duration::from_millis(100))?
            && let Event::Key(key) = event::read()?
            && key.kind == KeyEventKind::Press
        {
            app.handle_key(key, Instant::now());
        }
    }
    Ok(())
}
