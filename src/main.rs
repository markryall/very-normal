use std::io::{BufRead, IsTerminal, Read, Write};
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};
use ratatui::DefaultTerminal;
use ratatui::crossterm::event::{self, Event, KeyEventKind};
use very_normal::app::{App, Phase};
use very_normal::layout;
use very_normal::optimise::{self, Options};
use very_normal::store::{self, Progress};
use very_normal::{charts, coach, corpus, keymap, named, oryx, plans, report};

const HELP: &str = "\
very-normal: move your keyboard to a layout made from your own typing

usage: very-normal                  a guided session: layout, warm-up, practice
       very-normal dashboard        the trainer with everything on one screen
       very-normal status           what the coach thinks of your practice today
       very-normal next             the Oryx edits for the next stage, and where to make them
       very-normal check            read the layout from Oryx and compare it with your stage
       very-normal stage <n>        set the stage you're practising (0-9)
       very-normal baseline <wpm>   type in a baseline instead of recording one
       very-normal import <file|->  merge a backup copied out of the web tutor
       very-normal plan             which plan you're on, and its stages
       very-normal make-plan [...]  make a layout and plan from your own typing
                                    (very-normal make-plan --help for its options)
       very-normal use <name>       switch plans; each plan keeps its own progress
       very-normal remove <name>    delete a plan you made, its summary and its progress
       very-normal share [name]     share a plan's summary (no text) on GitHub
       very-normal keymap kanata|karabiner [stage] [--all-keyboards]
                                    a software keymap for keyboards without Oryx
                                    (karabiner: built-in keyboard only, unless --all-keyboards)
       very-normal charts [file] [--no-open]
                                    write your progress as a page of charts and open it
       very-normal export           print your progress as JSON
       very-normal --help | --version

Progress lives in ~/.local/share/very-normal/progress.json ($VERY_NORMAL_DATA).
Plans live in ~/.config/very-normal ($VERY_NORMAL_PLAN). The Oryx layout comes
from your plan ($VERY_NORMAL_LAYOUT); URLs open with `open`, or `xdg-open` on
Linux ($VERY_NORMAL_OPEN, where {url} marks the address).
";

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let path = store::default_path();
    if args.is_empty()
        && !layout::plan_path().exists()
        && !path.exists()
        && std::io::stdin().is_terminal()
        && !welcome(&path)?
    {
        return Ok(());
    }
    let plan_file = layout::install()?;
    let words: Vec<&str> = args.iter().map(String::as_str).collect();
    match words.as_slice() {
        [] | ["dashboard"] => {
            let phase = if words.is_empty() {
                Phase::Layout
            } else {
                Phase::Dashboard
            };
            let progress = Progress::load(&path)?;
            let mut terminal = ratatui::init();
            let result = run(
                &mut terminal,
                App::new(progress, path, Instant::now(), phase),
            );
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
                "today {}/{} sessions · {:.0} min · {}-day streak · practising stage {}",
                a.sessions_today,
                coach::SESSIONS_PER_DAY,
                a.today_minutes,
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
            let active = plans::active_slug();
            let others: Vec<String> = plans::list().into_iter().filter(|p| *p != active).collect();
            if !others.is_empty() {
                println!(
                    "\nOther plans: {} (switch with `very-normal use <name>`)",
                    others.join(", ")
                );
            }
        }
        ["use", name] => println!("{}", plans::switch(name, &path)?),
        ["remove", name] => println!("{}", plans::remove(name, &path)?),
        ["make-plan", "-h" | "--help"] => print!("{}", optimise::USAGE),
        ["make-plan", rest @ ..] => {
            let made = optimise::make(&Options::parse(rest)?)?;
            report_made(&made);
        }
        ["share"] => share(&latest_summary()?)?,
        ["share", name] => {
            let file = std::path::Path::new(name);
            if file.is_file() {
                share(file)?
            } else {
                share(&plans::summary_of(&plans::made_path(name)))?
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

/// The first run, with no plan and no progress yet: offers to make a layout
/// from your own typing. Says whether to carry on into the trainer.
fn welcome(progress: &std::path::Path) -> Result<bool> {
    let projects = corpus::claude_projects();
    let typed = corpus::typed_messages(&projects).len();
    println!("Welcome to very-normal.\n");
    println!("very-normal can make you a keyboard layout from your own typing, and a plan");
    println!("that moves you there from the layout you type on now, a few keys at a time.\n");
    let enough = typed >= optimise::ENOUGH;
    if enough {
        println!(
            "It found {typed} messages you typed to Claude Code in {}.",
            projects.display()
        );
        println!("(Only your messages: never Claude's replies, tool output or pastes.)\n");
    } else {
        println!(
            "It found {typed} messages you typed to Claude Code in {}; it needs {} or more.\n",
            projects.display(),
            optimise::ENOUGH
        );
    }
    println!("Anything else you've written makes the layout fit you better. Add files or");
    println!("folders of .md, .txt or .org files (and `--no-claude` to leave Claude Code out):\n");
    println!("  very-normal make-plan --from dvorak --text ~/notes ~/Documents/emails.txt\n");
    if enough {
        println!("  1  make my layout from my Claude Code messages now (about a minute)");
    }
    println!("  2  use Very Normal, the built-in layout, for now (it starts from QWERTY)");
    println!("  q  quit, and come back with make-plan\n");
    let choice = ask(if enough {
        "Choose [1]: "
    } else {
        "Choose [2]: "
    })?;
    let choice = match choice.as_str() {
        "" if enough => "1",
        "" => "2",
        c => c,
    };
    match choice {
        "1" if enough => {
            let mut o = Options {
                from: ask_start()?,
                ..Options::default()
            };
            let name = ask("What should your layout be called? [My Layout]: ")?;
            if !name.is_empty() {
                o.name = name;
            }
            println!();
            let made = optimise::make(&o)?;
            report_made(&made);
            if ask("\nShare your summary on GitHub? It has no text or words, just the layout and its scores. [y/N]: ")?
                .eq_ignore_ascii_case("y")
            {
                share(&made.summary_path)?;
            }
            Ok(ask("\nPress enter to start training, or q to quit: ")? != "q")
        }
        "2" => {
            Progress::default().save(progress)?;
            Ok(true)
        }
        _ => Ok(false),
    }
}

/// Which layout you type on now: one of the usual ones, or your own 30 keys.
fn ask_start() -> Result<String> {
    println!("\nWhich layout do you type on now?\n");
    for (i, name) in named::STARTS.iter().enumerate() {
        println!("  {}  {name}", i + 1);
    }
    println!("  or type your own 30 keys, top row first, like qwertyuiop asdfghjkl; zxcvbnm,./\n");
    loop {
        let answer = ask("Layout [1]: ")?;
        let pick = match answer.as_str() {
            "" => named::STARTS[0].to_string(),
            n if n
                .parse::<usize>()
                .is_ok_and(|n| (1..=named::STARTS.len()).contains(&n)) =>
            {
                named::STARTS[n.parse::<usize>()? - 1].to_string()
            }
            other => other.to_string(),
        };
        match named::start(&pick) {
            Ok((name, keys)) => {
                println!("\n{}\n", optimise::pretty(&keys));
                println!("Starting from {name}.");
                return Ok(pick);
            }
            Err(e) => println!("{e}"),
        }
    }
}

fn ask(prompt: &str) -> Result<String> {
    print!("{prompt}");
    std::io::stdout().flush()?;
    let mut line = String::new();
    std::io::stdin().lock().read_line(&mut line)?;
    Ok(line.trim().to_string())
}

fn report_made(made: &optimise::Made) {
    println!("{}", made.summary);
    println!(
        "Your plan ({} stages) is in {}. Its drill words come from your own writing,",
        made.stages,
        made.plan_path.display()
    );
    println!("so keep it private, and look through them for names you'd rather not see on screen.");
    println!(
        "The summary in {} has no words or text in it.\n",
        made.summary_path.display()
    );
    if !made.name_ideas.is_empty() {
        println!(
            "Name ideas you can type on your new home row: {}",
            made.name_ideas.join(", ")
        );
        println!("(Run make-plan again with --name; the same seed gives the same layout.)\n");
    }
    if made.activated {
        println!("It's the plan you're practising now.");
    } else {
        println!("Your current plan and progress are untouched. To practise this one:");
        println!("  very-normal use {}", made.slug);
        println!("Each plan keeps its own progress, so you can switch back any time.\n");
    }
    println!("Next: `very-normal plan` to see the stages, `very-normal` to start training,");
    println!("and `very-normal share` to add your layout to the collection. I'm curious how");
    println!("much everyone's layouts differ.");
}

/// The summary of the plan you made most recently.
fn latest_summary() -> Result<std::path::PathBuf> {
    std::fs::read_dir(plans::dir())
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "md"))
        .max_by_key(|p| std::fs::metadata(p).and_then(|m| m.modified()).ok())
        .ok_or_else(|| {
            anyhow::anyhow!("no summary to share; make a plan first with `very-normal make-plan`")
        })
}

/// Opens a new GitHub issue with the summary filled in, for you to check and post.
fn share(summary: &std::path::Path) -> Result<()> {
    let text = std::fs::read_to_string(summary).with_context(|| {
        format!(
            "reading {}; make a plan first with `very-normal make-plan`",
            summary.display()
        )
    })?;
    let url = optimise::share_url(&text);
    if url.len() > optimise::LONGEST_LINK {
        println!(
            "The summary is too long for a link. Open this page and paste in {}:",
            summary.display()
        );
        let short = optimise::share_url(text.lines().next().unwrap_or("# My layout"));
        println!("{short}");
        return very_normal::open_url(&short);
    }
    println!(
        "Opening a new issue with your summary filled in. Nothing is posted until you press Submit."
    );
    println!("{url}");
    very_normal::open_url(&url)
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
