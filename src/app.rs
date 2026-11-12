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
use crate::{coach, oryx, report};

const PINK: Color = Color::Magenta;
const TEAL: Color = Color::Cyan;

pub struct App {
    pub progress: Progress,
    path: PathBuf,
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
    pub should_quit: bool,
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

impl App {
    pub fn new(progress: Progress, path: PathBuf, now: Instant) -> App {
        let mut rng = Rng::new();
        let stage = progress.stage.min(layout::last());
        let drill = Drill::new(
            Mode::Words,
            stage,
            build(Mode::Words, stage, &progress, &mut rng),
            now,
        );
        App {
            progress,
            path,
            mode: Mode::Words,
            drill,
            typing: false,
            rng,
            message: None,
            check: Vec::new(),
            layout_id: oryx::layout_id(),
            override_armed: false,
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
        if self.typing {
            self.drill.tick(now);
            if self.drill.done {
                self.finish(now);
            }
        }
    }

    fn finish(&mut self, now: Instant) {
        self.typing = false;
        let (stage, mode) = (self.drill.stage, self.drill.mode);
        let (wpm, acc) = (self.drill.wpm(now), self.drill.accuracy());
        let t = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);
        let run = Run {
            t,
            stage,
            mode: mode.id().into(),
            wpm: (wpm * 10.0).round() / 10.0,
            acc: (acc * 10.0).round() / 10.0,
            secs: (self.drill.elapsed(now).as_secs_f64() * 10.0).round() / 10.0,
        };
        let letters = self.drill.letters().clone();
        self.progress.record(run, &letters);
        self.save();

        let mut good = true;
        let verdict = match mode {
            Mode::Words if stage > 0 => {
                good = acc >= gate::WORDS_ACCURACY;
                if good {
                    "Counts toward the gate.".to_string()
                } else {
                    "Under 97%, so the run of three starts again. Slow down; accuracy first."
                        .to_string()
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
        self.message = Some((
            format!("{wpm:.0} WPM · {acc:.1}% accuracy. {verdict}"),
            good,
        ));
    }

    pub fn handle_key(&mut self, key: KeyEvent, now: Instant) {
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            self.should_quit = true;
            return;
        }
        if self.typing {
            match key.code {
                KeyCode::Esc => {
                    self.typing = false;
                    self.message = Some(("Stopped. Nothing was saved.".into(), true));
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
                if self.drill.done || self.drill.started() {
                    self.new_drill(now);
                }
                self.message = None;
                self.drill.touch(now);
                self.typing = true;
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
        let help = if self.typing {
            "type the text · esc stops without saving"
        } else {
            "←/→ stage · 1-6 drill · enter start · n new text · c check oryx · o open oryx · k/v/w tick · q quit"
        };
        frame.render_widget(Paragraph::new(help).style(dim()), footer);
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
                    "   today {:.0}/{:.0} min · session {:.0} min",
                    advice.today_minutes,
                    coach::DAILY_GOAL,
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
                format!(" {}", layout::OUTER_RIGHT[row]),
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

    fn render_trainer(&self, frame: &mut Frame, area: Rect, now: Instant) {
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
        let mut lines = vec![
            Line::from(tabs),
            Line::styled(self.mode.describe(s), dim()),
            Line::raw(""),
        ];
        lines.extend(self.text_lines());
        lines.push(Line::raw(""));
        lines.push(self.status_line(now));
        if let Some((msg, good)) = &self.message {
            let color = if *good { Color::Green } else { Color::LightRed };
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

    fn status_line(&self, now: Instant) -> Line<'static> {
        let d = &self.drill;
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
            ..Default::default()
        };
        (App::new(p, path.clone(), Instant::now()), path)
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
            })
            .collect();
        let p = Progress {
            stage: 1,
            runs,
            ..Default::default()
        };
        let mut app = App::new(p, path.clone(), Instant::now());
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
