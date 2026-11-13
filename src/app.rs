//! The TUI: stage, keyboard map, next Oryx edits, gate, and the trainer.

use std::path::PathBuf;
use std::time::Instant;

use ratatui::Frame;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph, Wrap};

use crate::drill::{Drill, Mode, Rng, build};
use crate::gate;
use crate::layout;
use crate::store::{Progress, Run};
use crate::warmup::{self, Warmup};
use crate::{coach, oryx, report};

const PINK: Color = Color::Magenta;
const TEAL: Color = Color::Cyan;
/// Restarts this close together suggest frustration.
const ABANDON_WINDOW: std::time::Duration = std::time::Duration::from_secs(10 * 60);

/// Where you are in a session: the guided steps, then the dashboard.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    /// The layout on its own, before anything else.
    Layout,
    /// The breathing and stretches, every guided session.
    Warmup,
    /// Everything: stages, drills, the gate and Oryx.
    Dashboard,
}

pub struct App {
    pub progress: Progress,
    pub phase: Phase,
    path: PathBuf,
    /// The troublesome keys this drill's words lean on.
    leaning: Vec<char>,
    mode: Mode,
    drill: Drill,
    typing: bool,
    rng: Rng,
    /// The result of the last drill, or of the last thing you did.
    message: Option<(String, bool)>,
    /// The last `c` check against Oryx.
    check: Vec<String>,
    layout_id: String,
    /// Set when the coach said stop and you pressed enter anyway once.
    override_armed: bool,
    /// The warm-up, while it runs.
    warmup: Option<Warmup>,
    /// When you recently gave up on a drill with esc; several close together
    /// usually means frustration.
    abandons: Vec<Instant>,
    pub should_quit: bool,
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

impl App {
    pub fn new(progress: Progress, path: PathBuf, now: Instant, phase: Phase) -> App {
        let mut rng = Rng::new();
        let stage = progress.stage.min(layout::last());
        let leaning = crate::drill::leaning(Mode::Words, stage, &progress);
        let drill = Drill::new(
            Mode::Words,
            stage,
            build(Mode::Words, stage, &progress, &mut rng),
            now,
        );
        App {
            progress,
            phase,
            path,
            leaning,
            mode: Mode::Words,
            drill,
            typing: false,
            rng,
            message: None,
            check: Vec::new(),
            layout_id: oryx::layout_id(),
            override_armed: false,
            warmup: None,
            abandons: Vec::new(),
            should_quit: false,
        }
    }

    fn stage(&self) -> usize {
        self.progress.stage.min(layout::last())
    }

    fn save(&mut self) {
        if let Err(e) = self.progress.save(&self.path) {
            self.message = Some((format!("Couldn't save progress: {e}"), false));
        }
    }

    fn new_drill(&mut self, now: Instant) {
        let text = build(self.mode, self.stage(), &self.progress, &mut self.rng);
        self.leaning = crate::drill::leaning(self.mode, self.stage(), &self.progress);
        self.drill = Drill::new(self.mode, self.stage(), text, now);
    }

    fn set_stage(&mut self, stage: usize, now: Instant) {
        self.progress.stage = stage.min(layout::last());
        self.check.clear();
        self.message = None;
        self.save();
        self.new_drill(now);
    }

    /// Ends a timed drill once its minute is up.
    pub fn tick(&mut self, now: Instant) {
        if self.warmup.as_ref().is_some_and(|w| w.done(now)) {
            self.end_warmup(true);
        }
        if self.typing {
            self.drill.tick(now);
            if self.drill.done {
                self.finish(now);
            }
        }
    }

    fn needs_warmup(&self) -> bool {
        self.progress.warmed_up.as_deref() != Some(coach::today(now_ms()).as_str())
    }

    fn end_warmup(&mut self, completed: bool) {
        self.warmup = None;
        if self.phase == Phase::Warmup {
            self.phase = Phase::Dashboard;
        }
        self.progress.warmed_up = Some(coach::today(now_ms()));
        self.save();
        self.message = Some(if completed {
            (
                "Warmed up. Fingers loose, shoulders down. Press enter to start.".into(),
                true,
            )
        } else {
            (
                "Skipping the warm-up. That's fine; go gently for the first minute.".into(),
                true,
            )
        });
    }

    fn finish(&mut self, now: Instant) {
        self.typing = false;
        let (stage, mode) = (self.drill.stage, self.drill.mode);
        // round once, so the number shown, the number saved and the number judged agree
        let wpm = (self.drill.wpm(now) * 10.0).round() / 10.0;
        let acc = (self.drill.accuracy() * 10.0).round() / 10.0;
        let previous_best = self
            .progress
            .runs_for(stage, mode.id())
            .iter()
            .map(|r| r.wpm)
            .fold(None, |best: Option<f64>, w| {
                Some(best.map_or(w, |b| b.max(w)))
            });
        let t = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);
        let run = Run {
            t,
            stage,
            mode: mode.id().into(),
            wpm,
            acc,
            secs: (self.drill.elapsed(now).as_secs_f64() * 10.0).round() / 10.0,
            letters: self.drill.letters().clone(),
            typos: self.drill.typos().clone(),
        };
        self.progress.record(run, self.drill.pairs());
        self.save();

        let mut good = true;
        let verdict = match mode {
            Mode::Words if stage > 0 => {
                good = acc >= gate::WORDS_ACCURACY;
                let hits = gate::words_hits(&self.progress, stage).min(gate::WORDS_NEEDED);
                if good && hits >= gate::WORDS_NEEDED {
                    "That one counts, and that's the accuracy part of the gate done!".to_string()
                } else if good {
                    format!("That one counts: {hits} of {}.", gate::WORDS_NEEDED)
                } else {
                    "Not one for the gate this time. Every rep still teaches your fingers; slow and smooth wins.".to_string()
                }
            }
            Mode::Real if stage == 0 => "Saved toward your baseline.".to_string(),
            Mode::Real => match self.progress.baseline() {
                Some(b) => {
                    let target = b * gate::SPEED_SHARE;
                    good = wpm >= target && acc >= gate::REAL_ACCURACY;
                    format!("Target is {target:.0} WPM at 95%+.")
                }
                None => "Record a baseline in stage 0 to get a target.".to_string(),
            },
            Mode::Recall => {
                let slow: Vec<String> = self
                    .drill
                    .slowest(3)
                    .iter()
                    .map(|(c, ms, miss)| {
                        if *miss > 0 {
                            format!("{c} {ms:.0}ms, {miss} missed")
                        } else {
                            format!("{c} {ms:.0}ms")
                        }
                    })
                    .collect();
                format!("Slowest: {}", slow.join(" · "))
            }
            _ => String::new(),
        };
        let record = match previous_best {
            Some(best) if wpm > best && acc >= 90.0 => " New speed record for this stage!",
            _ => "",
        };
        self.message = Some((
            format!("{wpm:.0} WPM · {acc:.1}% accuracy. {verdict}{record}"),
            good,
        ));
    }

    pub fn handle_key(&mut self, key: KeyEvent, now: Instant) {
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            self.should_quit = true;
            return;
        }
        if self.phase == Phase::Layout {
            match key.code {
                KeyCode::Enter => {
                    self.phase = Phase::Warmup;
                    self.warmup = Some(Warmup::new(now));
                    self.message = None;
                }
                KeyCode::Char('d') => self.phase = Phase::Dashboard,
                KeyCode::Char('q') => self.should_quit = true,
                _ => {}
            }
            return;
        }
        if let Some(w) = &mut self.warmup {
            match key.code {
                KeyCode::Enter | KeyCode::Tab | KeyCode::Char(' ') => {
                    w.skip_step(now);
                    if w.done(now) {
                        self.end_warmup(true);
                    }
                }
                KeyCode::Char('s') => self.end_warmup(false),
                KeyCode::Esc => {
                    self.warmup = None;
                    self.phase = Phase::Dashboard;
                    self.message = Some((
                        "Warm-up paused. Press enter when you're ready.".into(),
                        true,
                    ));
                }
                _ => {}
            }
            return;
        }
        if self.typing {
            match key.code {
                KeyCode::Esc => {
                    self.typing = false;
                    self.abandons
                        .retain(|t| now.duration_since(*t) < ABANDON_WINDOW);
                    self.abandons.push(now);
                    self.message = Some(if self.abandons.len() >= 3 {
                        (
                            "That's a few restarts in a row. Frustration makes fingers tense, and tense fingers miss. Take five and come back loose.".into(),
                            false,
                        )
                    } else {
                        ("Stopped. Nothing was saved.".into(), true)
                    });
                    self.new_drill(now);
                }
                KeyCode::Char(c)
                    if !key
                        .modifiers
                        .intersects(KeyModifiers::ALT | KeyModifiers::SUPER) =>
                {
                    self.drill.press(c, now);
                    if self.drill.done {
                        self.finish(now);
                    }
                }
                _ => {}
            }
            return;
        }
        let stage = self.stage();
        match key.code {
            KeyCode::Char('q') => self.should_quit = true,
            KeyCode::Left | KeyCode::Char('[') if stage > 0 => self.set_stage(stage - 1, now),
            KeyCode::Right | KeyCode::Char(']') if stage < layout::last() => {
                self.set_stage(stage + 1, now)
            }
            KeyCode::Char(d @ '1'..='6') => {
                self.mode = Mode::ALL[d as usize - '1' as usize];
                self.message = None;
                self.new_drill(now);
            }
            KeyCode::Enter | KeyCode::Tab | KeyCode::Char(' ') => {
                let advice = self.advice();
                if advice.blocks() && !self.override_armed {
                    self.override_armed = true;
                    self.message = Some((
                        format!(
                            "{} Press enter again to practise anyway, rebel.",
                            advice.message
                        ),
                        false,
                    ));
                    return;
                }
                self.override_armed = false;
                if self.needs_warmup() {
                    self.warmup = Some(Warmup::new(now));
                    self.message = None;
                    return;
                }
                if self.drill.done || self.drill.started() {
                    self.new_drill(now);
                }
                self.message = None;
                self.drill.touch(now);
                self.typing = true;
            }
            KeyCode::Char('u') => {
                self.warmup = Some(Warmup::new(now));
                self.message = None;
            }
            KeyCode::Char('n') => {
                self.message = None;
                self.new_drill(now);
            }
            KeyCode::Char('c') => {
                self.check = match oryx::fetch(&self.layout_id) {
                    Ok(snap) => report::check(&snap, stage),
                    Err(e) => vec![format!("Couldn't read Oryx: {e}")],
                };
            }
            KeyCode::Char('g') => {
                let out = crate::charts::default_path(&self.path);
                let now_ms = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_millis() as u64)
                    .unwrap_or(0);
                self.message = Some(
                    match crate::charts::write(&self.progress, &out, now_ms)
                        .and_then(|()| crate::open_url(&out.display().to_string()))
                    {
                        Ok(()) => (format!("Charts opened from {}", out.display()), true),
                        Err(e) => (format!("Couldn't make the charts: {e}"), false),
                    },
                );
            }
            KeyCode::Char('o') => {
                let url = oryx::edit_url(&self.layout_id);
                self.message = Some(match crate::open_url(&url) {
                    Ok(()) => (format!("Opened {url}"), true),
                    Err(e) => (format!("Couldn't open {url}: {e}"), false),
                });
            }
            KeyCode::Char(c) => {
                if let Some(tick) = gate::items(&self.progress, stage)
                    .iter()
                    .filter_map(|i| i.tick)
                    .find(|t| t.key() == c)
                {
                    tick.toggle(&mut self.progress, stage);
                    self.save();
                }
            }
            _ => {}
        }
    }

    pub fn render(&self, frame: &mut Frame, now: Instant) {
        match self.phase {
            Phase::Layout => return self.render_layout(frame, now),
            Phase::Warmup => {
                let [main, footer] = Layout::vertical([Constraint::Fill(1), Constraint::Length(1)])
                    .areas(frame.area());
                if let Some(w) = &self.warmup {
                    self.render_warmup(frame, main, w, now);
                }
                frame.render_widget(
                    Paragraph::new("enter next step · s skip the warm-up · esc to the dashboard")
                        .style(dim()),
                    footer,
                );
                return;
            }
            Phase::Dashboard => {}
        }
        // tall enough for the next stage's edits (plus any check) and the gate
        let next_lines = report::next_stage_compact(self.stage(), "").len()
            + if self.check.is_empty() {
                0
            } else {
                self.check.len() + 1
            };
        let gate_lines = gate::items(&self.progress, self.stage())
            .iter()
            .map(|i| 1 + usize::from(!i.detail.is_empty()))
            .sum::<usize>()
            + 1;
        // the trainer keeps at least 8 rows; the middle panels take what is left
        let spare = frame.area().height.saturating_sub(3 + 1 + 5 + 1 + 8);
        let middle_height = ((next_lines.max(gate_lines) + 2).min(18) as u16).min(spare);
        let [header, blurb, keyboard, middle, trainer, footer] = Layout::vertical([
            Constraint::Length(3),
            Constraint::Length(1),
            Constraint::Length(5),
            Constraint::Length(middle_height),
            Constraint::Fill(1),
            Constraint::Length(1),
        ])
        .areas(frame.area());
        self.render_header(frame, header);
        frame.render_widget(
            Paragraph::new(layout::blurb(self.stage()))
                .style(dim())
                .wrap(Wrap { trim: true }),
            blurb,
        );
        self.render_keyboard(frame, keyboard, now);
        let [next, gate] =
            Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)])
                .areas(middle);
        self.render_next(frame, next);
        self.render_gate(frame, gate);
        self.render_trainer(frame, trainer, now);
        let help = if self.warmup.is_some() {
            "enter next step · s skip the warm-up · esc pause"
        } else if self.typing {
            "type the text · esc stops without saving"
        } else {
            "←/→ stage · 1-6 drill · enter start · u warm-up · n new text · g charts · c check oryx · o open oryx · k/v/w tick · q quit"
        };
        frame.render_widget(Paragraph::new(help).style(dim()), footer);
    }

    /// The guided session's first screen: just the layout and what's new in it.
    fn render_layout(&self, frame: &mut Frame, now: Instant) {
        let s = self.stage();
        let [title, _, keyboard, _, new_keys, _, footer] = Layout::vertical([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(5),
            Constraint::Length(1),
            Constraint::Length(2),
            Constraint::Fill(1),
            Constraint::Length(1),
        ])
        .areas(frame.area());
        frame.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled(
                    format!("{}  ", layout::plan().name.to_lowercase()),
                    Style::new().fg(PINK).add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!("stage {s} of {} · {}", layout::last(), layout::name(s)),
                    Style::new().add_modifier(Modifier::BOLD),
                ),
            ])),
            title,
        );
        self.render_keyboard(frame, keyboard, now);
        let moved: Vec<String> = layout::changes(s)
            .iter()
            .map(|c| c.to.to_string())
            .collect();
        let text = if moved.is_empty() {
            "Nothing moves this stage. Settle in and type.".to_string()
        } else {
            format!(
                "Moving this stage: {}. Take a look at where they live.",
                moved.join(", ")
            )
        };
        frame.render_widget(
            Paragraph::new(text)
                .style(Style::new().fg(PINK))
                .wrap(Wrap { trim: true }),
            new_keys,
        );
        frame.render_widget(
            Paragraph::new("enter warm up · d dashboard · q quit").style(dim()),
            footer,
        );
    }

    fn render_header(&self, frame: &mut Frame, area: Rect) {
        let s = self.stage();
        let (home, sfb) = layout::stats(s);
        let title = Line::from(vec![
            Span::styled(
                format!("{}  ", layout::plan().name.to_lowercase()),
                Style::new().fg(PINK).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("stage {s} of {} · {}", layout::last(), layout::name(s)),
                Style::new().add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("   {home}% home row · {sfb}% same-finger pairs"),
                dim(),
            ),
        ]);
        let mut steps = Vec::new();
        for i in 0..=layout::last() {
            let passed = gate::passed(&self.progress, i);
            let style = if i == s {
                Style::new()
                    .fg(Color::Black)
                    .bg(PINK)
                    .add_modifier(Modifier::BOLD)
            } else if passed {
                Style::new().fg(Color::Green)
            } else {
                dim()
            };
            steps.push(Span::styled(
                format!(" {i}{} ", if passed { "✓" } else { "" }),
                style,
            ));
            steps.push(Span::raw(" "));
        }
        let advice = self.advice();
        let (icon, color) = match advice.level {
            coach::Level::Good => ("✓", Color::Green),
            coach::Level::Nudge => ("»", Color::Yellow),
            coach::Level::Stop => ("✋", Color::LightRed),
        };
        let coach_line = Line::from(vec![
            Span::styled(
                format!("{icon} {}", advice.message),
                Style::new().fg(color).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!(
                    "   today {}/{} sessions · session {:.0} min",
                    advice.sessions_today,
                    coach::SESSIONS_PER_DAY,
                    advice.session_minutes
                ),
                dim(),
            ),
        ]);
        frame.render_widget(
            Paragraph::new(vec![title, Line::from(steps), coach_line]),
            area,
        );
    }

    pub fn advice(&self) -> coach::Advice {
        coach::assess(&self.progress.runs, now_ms())
    }

    fn render_keyboard(&self, frame: &mut Frame, area: Rect, now: Instant) {
        let s = self.stage();
        let keys = layout::keys(s);
        let mirror_mode = self.mode == Mode::Mirror;
        let hint = if self.typing && self.drill.hesitating(now) {
            self.drill.expected()
        } else {
            None
        };
        let hinted = |index: usize| -> bool {
            let Some(h) = hint else { return false };
            if mirror_mode {
                index % 10 < 5 && (keys[index] == h || keys[layout::mirror(index)] == h)
            } else {
                keys[index] == h
            }
        };
        let mut lines = Vec::new();
        for row in 0..3 {
            let mut spans = vec![Span::styled(
                format!("{:>4} ", layout::OUTER_LEFT[row]),
                dim(),
            )];
            for col in 0..10 {
                let i = row * 10 + col;
                if col == 5 {
                    spans.push(Span::raw("     "));
                }
                let mut style = if layout::changed_now(s, i) {
                    Style::new().fg(PINK).add_modifier(Modifier::BOLD)
                } else if layout::moved(s, i) {
                    Style::new().fg(TEAL)
                } else {
                    Style::new()
                };
                if hinted(i) {
                    style = Style::new()
                        .fg(Color::Black)
                        .bg(Color::Yellow)
                        .add_modifier(Modifier::BOLD);
                }
                if mirror_mode && col < 5 {
                    spans.push(Span::styled(format!(" {}", keys[i]), style));
                    spans.push(Span::styled(
                        format!("{} ", keys[layout::mirror(i)]),
                        Style::new().fg(PINK),
                    ));
                } else {
                    spans.push(Span::styled(format!(" {} ", keys[i]), style));
                }
            }
            spans.push(Span::styled(
                format!(" {}", layout::outer_right(row)),
                dim(),
            ));
            lines.push(Line::from(spans));
        }
        let legend = Line::from(vec![
            Span::styled("pink", Style::new().fg(PINK)),
            Span::styled(" moves this stage · ", dim()),
            Span::styled("teal", Style::new().fg(TEAL)),
            Span::styled(" moved earlier · yellow: the key you're looking for", dim()),
        ]);
        let block = Block::bordered().title(" voyager ").title_bottom(legend);
        frame.render_widget(Paragraph::new(lines).block(block), area);
    }

    fn render_next(&self, frame: &mut Frame, area: Rect) {
        let s = self.stage();
        let url = oryx::edit_url(&self.layout_id);
        let mut lines: Vec<Line> = report::next_stage_compact(s, &url)
            .into_iter()
            .map(Line::from)
            .collect();
        if !self.check.is_empty() {
            lines.push(Line::raw(""));
            lines.extend(
                self.check
                    .iter()
                    .map(|l| Line::styled(l.clone(), Style::new().fg(TEAL))),
            );
        }
        let block = Block::bordered().title(" next oryx edits (o open · c check) ");
        frame.render_widget(
            Paragraph::new(lines)
                .block(block)
                .wrap(Wrap { trim: false }),
            area,
        );
    }

    fn render_gate(&self, frame: &mut Frame, area: Rect) {
        let s = self.stage();
        let mut lines = Vec::new();
        for item in gate::items(&self.progress, s) {
            let mark = if item.ok {
                Span::styled("[✓] ", Style::new().fg(Color::Green))
            } else {
                Span::styled("[ ] ", dim())
            };
            let key = item
                .tick
                .map_or(String::new(), |t| format!(" ({})", t.key()));
            lines.push(Line::from(vec![
                mark,
                Span::raw(item.label),
                Span::styled(key, Style::new().fg(PINK)),
            ]));
            if !item.detail.is_empty() {
                lines.push(Line::styled(format!("    {}", item.detail), dim()));
            }
        }
        if gate::passed(&self.progress, s) {
            let msg = if s < layout::last() {
                format!(
                    "Gate passed. Make the stage {} edits, flash, then →.",
                    s + 1
                )
            } else {
                format!("Every gate passed. You type {}.", layout::plan().name)
            };
            lines.push(Line::styled(msg, Style::new().fg(Color::Green)));
        }
        let block = Block::bordered().title(format!(
            " gate before stage {} ",
            (s + 1).min(layout::last())
        ));
        frame.render_widget(
            Paragraph::new(lines)
                .block(block)
                .wrap(Wrap { trim: false }),
            area,
        );
    }

    fn render_warmup(&self, frame: &mut Frame, area: Rect, w: &Warmup, now: Instant) {
        let current = w.current(now);
        let mut lines = vec![
            Line::styled(
                format!(
                    "About {} seconds. Gentle: nothing should hurt.",
                    Warmup::total()
                ),
                dim(),
            ),
            Line::raw(""),
        ];
        for (i, step) in warmup::STEPS.iter().enumerate() {
            let line = match current {
                Some((c, left)) if c == i => Line::from(vec![
                    Span::styled(
                        format!(" ▶ {} ", step.title),
                        Style::new()
                            .fg(Color::Black)
                            .bg(PINK)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(format!("  {left}s  "), Style::new().fg(PINK)),
                    Span::raw(step.how),
                ]),
                Some((c, _)) if c > i => {
                    Line::styled(format!(" ✓ {}", step.title), Style::new().fg(Color::Green))
                }
                _ => Line::styled(format!("   {} ({}s)", step.title, step.secs), dim()),
            };
            lines.push(line);
        }
        if let Some(b) = w.breath(now) {
            lines.push(Line::raw(""));
            lines.push(Line::styled(
                format!("        {b}"),
                Style::new().fg(TEAL).add_modifier(Modifier::BOLD),
            ));
        }
        let block = Block::bordered()
            .border_style(Style::new().fg(TEAL))
            .title(" warm-up ");
        frame.render_widget(
            Paragraph::new(lines)
                .block(block)
                .wrap(Wrap { trim: false }),
            area,
        );
    }

    fn render_trainer(&self, frame: &mut Frame, area: Rect, now: Instant) {
        if let Some(w) = &self.warmup {
            self.render_warmup(frame, area, w, now);
            return;
        }
        let s = self.stage();
        let mut tabs = Vec::new();
        for (n, m) in Mode::ALL.iter().enumerate() {
            let style = if *m == self.mode {
                Style::new()
                    .fg(Color::Black)
                    .bg(Color::White)
                    .add_modifier(Modifier::BOLD)
            } else {
                dim()
            };
            tabs.push(Span::styled(format!(" {} {} ", n + 1, m.label()), style));
            tabs.push(Span::raw(" "));
        }
        let mut lines = vec![Line::from(tabs), Line::styled(self.mode.describe(s), dim())];
        if let Some(line) = self.leaning_line() {
            lines.push(line);
        }
        lines.push(Line::raw(""));
        lines.extend(self.text_lines());
        lines.push(Line::raw(""));
        lines.push(self.status_line(now));
        if let Some((msg, good)) = &self.message {
            // misses are yellow, not red: they're information, not a telling-off
            let color = if *good { Color::Green } else { Color::Yellow };
            lines.push(Line::styled(msg.clone(), Style::new().fg(color)));
        }
        let recent: Vec<_> = self.progress.runs.iter().rev().take(3).collect();
        if !recent.is_empty() {
            lines.push(Line::raw(""));
            for r in recent {
                lines.push(Line::styled(
                    format!(
                        "  stage {} · {:<14} {:>3.0} wpm  {:>5.1}%",
                        r.stage,
                        Mode::label_for(&r.mode),
                        r.wpm,
                        r.acc
                    ),
                    dim(),
                ));
            }
        }
        let border = if self.typing {
            Style::new().fg(PINK)
        } else {
            Style::new()
        };
        let block = Block::bordered().border_style(border).title(" trainer ");
        frame.render_widget(
            Paragraph::new(lines)
                .block(block)
                .wrap(Wrap { trim: false }),
            area,
        );
    }

    fn text_lines(&self) -> Vec<Line<'static>> {
        let d = &self.drill;
        if d.done {
            return vec![Line::styled("Done. Press enter for another round.", dim())];
        }
        if d.mode == Mode::Recall {
            let cur = d.expected().unwrap_or(' ');
            let next: String = d
                .text
                .iter()
                .skip(d.pos + 1)
                .take(3)
                .flat_map(|c| [*c, ' '])
                .collect();
            let style = if d.error {
                Style::new()
                    .fg(Color::Black)
                    .bg(Color::LightRed)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::new()
                    .fg(Color::Black)
                    .bg(PINK)
                    .add_modifier(Modifier::BOLD)
            };
            return vec![Line::from(vec![
                Span::raw("      "),
                Span::styled(format!("   {cur}   "), style),
                Span::styled(format!("    next: {next}"), dim()),
            ])];
        }
        let mut start = d.pos.saturating_sub(40);
        while start > 0 && d.text[start - 1] != ' ' {
            start -= 1;
        }
        let end = (start + 240).min(d.text.len());
        let mut spans = Vec::new();
        for i in start..end {
            let c = d.text[i];
            let style = if i < d.pos {
                Style::new()
            } else if i == d.pos {
                let base = Style::new().add_modifier(Modifier::UNDERLINED | Modifier::BOLD);
                if d.error {
                    base.bg(Color::LightRed).fg(Color::Black)
                } else {
                    base.fg(PINK)
                }
            } else {
                dim()
            };
            let shown = if c == ' ' && i == d.pos { '·' } else { c };
            spans.push(Span::styled(shown.to_string(), style));
        }
        vec![Line::from(spans)]
    }

    /// Names the keys the words lean on: extra reps, said kindly.
    fn leaning_line(&self) -> Option<Line<'static>> {
        let keys: Vec<String> = self.leaning.iter().map(char::to_string).collect();
        let named = match keys.as_slice() {
            [] => return None,
            [one] => one.clone(),
            [rest @ .., last] => format!("{} and {last}", rest.join(", ")),
        };
        Some(Line::styled(
            format!("A few extra words with {named} in them, your trickiest keys right now."),
            dim(),
        ))
    }

    fn status_line(&self, now: Instant) -> Line<'static> {
        let d = &self.drill;
        if !self.typing && self.needs_warmup() {
            return Line::styled(
                "Press enter to start. First session today, so a one-minute warm-up comes first.",
                dim(),
            );
        }
        if !self.typing {
            return Line::styled(
                "Press enter to start. If you hesitate, the key lights up on the map.",
                dim(),
            );
        }
        if !d.started() {
            return Line::styled(
                "Go! The timer starts on your first key.",
                Style::new().fg(PINK),
            );
        }
        let progress = match d.remaining(now) {
            Some(r) => format!("{}s left", r.as_secs() + u64::from(r.subsec_millis() > 0)),
            None => format!("{}/{}", d.pos, d.text.len()),
        };
        Line::styled(
            format!(
                "{progress} · {:.0} wpm · {:.1}% accuracy",
                d.wpm(now),
                d.accuracy()
            ),
            Style::new().fg(TEAL),
        )
    }
}

fn dim() -> Style {
    Style::new().fg(Color::DarkGray)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use ratatui::crossterm::event::KeyEventKind;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new_with_kind(code, KeyModifiers::NONE, KeyEventKind::Press)
    }

    fn app() -> (App, PathBuf) {
        let path = std::env::temp_dir().join(format!(
            "very-normal-app-{}-{:?}.json",
            std::process::id(),
            std::thread::current().id()
        ));
        let p = Progress {
            stage: 1,
            warmed_up: Some(coach::today(now_ms())),
            ..Default::default()
        };
        (
            App::new(p, path.clone(), Instant::now(), Phase::Dashboard),
            path,
        )
    }

    fn screen(app: &App) -> String {
        let mut t = Terminal::new(TestBackend::new(200, 40)).unwrap();
        t.draw(|f| app.render(f, Instant::now())).unwrap();
        t.backend()
            .buffer()
            .content()
            .iter()
            .map(|c| c.symbol())
            .collect::<String>()
    }

    #[test]
    fn shows_the_stage_and_the_next_edits() {
        let (app, _) = app();
        let s = screen(&app);
        assert!(s.contains("stage 1 of 9"));
        assert!(s.contains("Stage 2: I and O come home"));
        assert!(s.contains("configure.zsa.io/voyager/layouts/"));
        assert!(s.contains("keys: i→a  o→k  a→i  k→o (keep alt)"));
    }

    #[test]
    fn typing_a_whole_drill_records_a_run() {
        let (mut app, path) = app();
        let now = Instant::now();
        app.handle_key(key(KeyCode::Enter), now);
        let text: Vec<char> = app.drill.text.clone();
        for c in text {
            app.handle_key(key(KeyCode::Char(c)), now);
        }
        assert!(!app.typing);
        assert_eq!(app.progress.runs.len(), 1);
        assert_eq!(app.progress.runs[0].mode, "words");
        assert_eq!(app.progress.runs[0].acc, 100.0);
        assert!(Progress::load(&path).unwrap().runs.len() == 1);
        std::fs::remove_file(path).ok();
    }

    #[test]
    fn letters_typed_outside_a_drill_tick_boxes_instead() {
        let (mut app, path) = app();
        app.handle_key(key(KeyCode::Char('v')), Instant::now());
        assert!(app.progress.manual(1).vim);
        app.handle_key(key(KeyCode::Right), Instant::now());
        assert_eq!(app.progress.stage, 2);
        std::fs::remove_file(path).ok();
    }

    #[test]
    fn escape_abandons_a_drill_without_saving() {
        let (mut app, path) = app();
        let now = Instant::now();
        app.handle_key(key(KeyCode::Enter), now);
        app.handle_key(key(KeyCode::Char('x')), now);
        app.handle_key(key(KeyCode::Esc), now);
        assert!(!app.typing);
        assert!(app.progress.runs.is_empty());
        std::fs::remove_file(path).ok();
    }
}

#[cfg(test)]
mod coach_tests {
    use super::*;
    use ratatui::crossterm::event::KeyEventKind;

    #[test]
    fn the_coach_makes_you_ask_twice_after_a_long_session() {
        let path =
            std::env::temp_dir().join(format!("very-normal-coach-{}.json", std::process::id()));
        let end = now_ms();
        let runs = (0..13u64)
            .map(|i| Run {
                t: end - (12 - i) * 120_000,
                stage: 1,
                mode: "words".into(),
                wpm: 15.0,
                acc: 98.0,
                secs: 120.0,
                ..Run::default()
            })
            .collect();
        let p = Progress {
            stage: 1,
            runs,
            warmed_up: Some(coach::today(now_ms())),
            ..Default::default()
        };
        let mut app = App::new(p, path.clone(), Instant::now(), Phase::Dashboard);
        let enter =
            KeyEvent::new_with_kind(KeyCode::Enter, KeyModifiers::NONE, KeyEventKind::Press);
        app.handle_key(enter, Instant::now());
        assert!(!app.typing);
        assert!(app.message.as_ref().unwrap().0.contains("rebel"));
        app.handle_key(enter, Instant::now());
        assert!(app.typing);
        std::fs::remove_file(path).ok();
    }
}

#[cfg(test)]
mod warmup_tests {
    use super::*;
    use ratatui::crossterm::event::KeyEventKind;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new_with_kind(code, KeyModifiers::NONE, KeyEventKind::Press)
    }

    #[test]
    fn the_first_drill_of_the_day_starts_with_a_warm_up() {
        let path =
            std::env::temp_dir().join(format!("very-normal-warmup-{}.json", std::process::id()));
        let mut app = App::new(
            Progress {
                stage: 1,
                ..Default::default()
            },
            path.clone(),
            Instant::now(),
            Phase::Dashboard,
        );
        let now = Instant::now();
        app.handle_key(key(KeyCode::Enter), now);
        assert!(app.warmup.is_some() && !app.typing);

        app.handle_key(key(KeyCode::Char('s')), now);
        assert!(app.warmup.is_none());
        let skipped = &app.message.as_ref().unwrap().0;
        assert!(
            !skipped.contains("Bold") && !skipped.contains("wrists"),
            "{skipped}"
        );
        assert_eq!(app.progress.warmed_up, Some(coach::today(now_ms())));

        app.handle_key(key(KeyCode::Enter), now);
        assert!(app.typing, "no second warm-up the same day");
        std::fs::remove_file(path).ok();
    }

    #[test]
    fn enter_steps_through_the_warm_up_to_the_end() {
        let path =
            std::env::temp_dir().join(format!("very-normal-warmup2-{}.json", std::process::id()));
        let mut app = App::new(
            Progress::default(),
            path.clone(),
            Instant::now(),
            Phase::Dashboard,
        );
        let now = Instant::now();
        app.handle_key(key(KeyCode::Char('u')), now);
        for _ in 0..warmup::STEPS.len() {
            app.handle_key(key(KeyCode::Enter), now);
        }
        assert!(app.warmup.is_none());
        assert!(app.message.as_ref().unwrap().0.contains("Warmed up"));
        std::fs::remove_file(path).ok();
    }
}

#[cfg(test)]
mod verdict_tests {
    use super::*;
    use crate::store::Run;

    fn words_run(acc: f64) -> Run {
        Run {
            t: 1,
            stage: 1,
            mode: "words".into(),
            wpm: 15.0,
            acc,
            secs: 60.0,
            ..Run::default()
        }
    }

    #[test]
    fn the_number_shown_is_the_number_judged() {
        // 96.96% shows as 97.0%, so it has to count as 97%
        let p = Progress {
            runs: vec![words_run((96.96f64 * 10.0).round() / 10.0)],
            ..Default::default()
        };
        assert_eq!(gate::words_hits(&p, 1), 1);
    }
}

#[cfg(test)]
mod frustration_tests {
    use super::*;
    use ratatui::crossterm::event::KeyEventKind;

    #[test]
    fn several_restarts_suggest_a_break() {
        let path = std::env::temp_dir().join(format!(
            "very-normal-frustration-{}.json",
            std::process::id()
        ));
        let p = Progress {
            stage: 1,
            warmed_up: Some(coach::today(now_ms())),
            ..Default::default()
        };
        let mut app = App::new(p, path.clone(), Instant::now(), Phase::Dashboard);
        let key = |c| KeyEvent::new_with_kind(c, KeyModifiers::NONE, KeyEventKind::Press);
        for _ in 0..3 {
            app.handle_key(key(KeyCode::Enter), Instant::now());
            app.handle_key(key(KeyCode::Esc), Instant::now());
        }
        assert!(app.message.as_ref().unwrap().0.contains("Frustration"));
        std::fs::remove_file(path).ok();
    }
}

#[cfg(test)]
mod guided_tests {
    use super::*;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use ratatui::buffer::Buffer;
    use ratatui::crossterm::event::KeyEventKind;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new_with_kind(code, KeyModifiers::NONE, KeyEventKind::Press)
    }

    fn ctrl_c() -> KeyEvent {
        KeyEvent::new_with_kind(
            KeyCode::Char('c'),
            KeyModifiers::CONTROL,
            KeyEventKind::Press,
        )
    }

    fn guided(name: &str, warmed_up: Option<String>) -> App {
        let path = std::env::temp_dir().join(format!(
            "very-normal-guided-{name}-{}.json",
            std::process::id()
        ));
        let p = Progress {
            stage: 1,
            warmed_up,
            ..Default::default()
        };
        App::new(p, path, Instant::now(), Phase::Layout)
    }

    fn draw(app: &App) -> Buffer {
        let mut t = Terminal::new(TestBackend::new(80, 24)).unwrap();
        t.draw(|f| app.render(f, Instant::now())).unwrap();
        t.backend().buffer().clone()
    }

    fn text(b: &Buffer) -> String {
        let w = b.area.width as usize;
        b.content()
            .chunks(w)
            .map(|row| row.iter().map(|c| c.symbol()).collect::<String>())
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn assert_quiet(s: &str, app: &App) {
        assert!(
            !s.contains(&app.advice().message),
            "coach message on screen:\n{s}"
        );
        assert!(!s.to_lowercase().contains("gate"), "gate on screen:\n{s}");
        assert!(!s.to_lowercase().contains("oryx"), "oryx on screen:\n{s}");
        for m in Mode::ALL {
            assert!(!s.contains(m.label()), "{:?} on screen:\n{s}", m.label());
        }
    }

    #[test]
    fn starts_with_the_layout_on_its_own() {
        let app = guided("layout", None);
        let b = draw(&app);
        let s = text(&b);
        assert!(s.contains(&layout::plan().name.to_lowercase()), "{s}");
        assert!(s.contains("stage 1"), "{s}");
        assert!(s.contains(layout::name(1)), "{s}");
        assert!(s.contains("voyager"), "{s}");
        for c in layout::changes(1) {
            assert!(s.contains("Moving this stage"), "{s}");
            assert!(s.contains(c.to), "{s}");
        }
        let help = s.lines().last().unwrap();
        for k in ["enter", "d ", "q "] {
            assert!(help.contains(k), "{help}");
        }
        assert_quiet(&s, &app);
    }

    #[test]
    fn the_keys_that_move_stand_out() {
        let app = guided("styles", None);
        let b = draw(&app);
        let keys = layout::keys(1);
        let changed: Vec<char> = (0..30)
            .filter(|&i| layout::changed_now(1, i))
            .map(|i| keys[i])
            .collect();
        let still = (0..30)
            .find(|&i| !layout::moved(1, i))
            .map(|i| keys[i])
            .unwrap();
        // the key cells sit inside the border, between the outer-column labels
        let mut changed_fg = Vec::new();
        let mut still_fg = Vec::new();
        for y in 3..6 {
            for x in 6..41 {
                let cell = &b[(x, y)];
                let ch = cell.symbol().chars().next().unwrap_or(' ');
                if changed.contains(&ch) {
                    changed_fg.push(cell.fg);
                }
                if ch == still {
                    still_fg.push(cell.fg);
                }
            }
        }
        assert!(
            !changed_fg.is_empty() && changed_fg.iter().all(|&c| c == PINK),
            "{changed_fg:?}"
        );
        assert!(
            !still_fg.is_empty() && still_fg.iter().all(|&c| c != PINK),
            "{still_fg:?}"
        );
    }

    #[test]
    fn enter_starts_the_warm_up_even_after_one_today() {
        let mut app = guided("enter", Some(coach::today(now_ms())));
        app.handle_key(key(KeyCode::Enter), Instant::now());
        assert_eq!(app.phase, Phase::Warmup);
        assert!(app.warmup.is_some());
        let s = text(&draw(&app));
        assert!(s.contains("warm-up"), "{s}");
        assert!(!s.contains("voyager"), "{s}");
        assert_quiet(&s, &app);
    }

    #[test]
    fn d_goes_to_the_dashboard_and_q_quits() {
        let mut app = guided("d", None);
        app.handle_key(key(KeyCode::Char('d')), Instant::now());
        assert_eq!(app.phase, Phase::Dashboard);

        let mut app = guided("q", None);
        app.handle_key(key(KeyCode::Char('q')), Instant::now());
        assert!(app.should_quit);
    }

    #[test]
    fn finishing_the_warm_up_lands_on_the_dashboard_without_another() {
        let mut app = guided("finish", None);
        let now = Instant::now();
        app.handle_key(key(KeyCode::Enter), now);
        app.tick(now + std::time::Duration::from_secs(Warmup::total() + 1));
        assert_eq!(app.phase, Phase::Dashboard);
        assert_eq!(app.progress.warmed_up, Some(coach::today(now_ms())));

        let later = now + std::time::Duration::from_secs(Warmup::total() + 2);
        app.handle_key(key(KeyCode::Enter), later);
        assert!(app.warmup.is_none());
        assert!(app.typing);
    }

    #[test]
    fn skipping_or_leaving_the_warm_up() {
        let mut app = guided("skip", None);
        app.handle_key(key(KeyCode::Enter), Instant::now());
        app.handle_key(key(KeyCode::Char('s')), Instant::now());
        assert_eq!(app.phase, Phase::Dashboard);
        assert_eq!(app.progress.warmed_up, Some(coach::today(now_ms())));

        let mut app = guided("esc", None);
        app.handle_key(key(KeyCode::Enter), Instant::now());
        app.handle_key(key(KeyCode::Esc), Instant::now());
        assert_eq!(app.phase, Phase::Dashboard);
        assert_eq!(app.progress.warmed_up, None);
    }

    #[test]
    fn ctrl_c_quits_in_every_phase() {
        for phase in [Phase::Layout, Phase::Warmup, Phase::Dashboard] {
            let mut app = guided("ctrlc", None);
            app.phase = phase;
            if phase == Phase::Warmup {
                app.warmup = Some(Warmup::new(Instant::now()));
            }
            app.handle_key(ctrl_c(), Instant::now());
            assert!(app.should_quit, "{phase:?}");
        }
    }
}
