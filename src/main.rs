pub static ARGS: LazyLock<Args> = LazyLock::new(Args::parse);

//use ratatui::{DefaultTerminal, Frame};
use std::cell::Cell;
use std::fs::File;
use std::io;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::LazyLock;

use clap::Parser;
use crossterm::event::{Event, EventStream, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use futures::StreamExt;
use ratatui::{
    DefaultTerminal, Frame,
    buffer::Buffer,
    layout::{Alignment, Constraint, Position, Rect},
    style::Stylize,
    symbols::border,
    text::{Line, Text},
    widgets::{Block, Padding, Paragraph, Widget},
};
use tracing_subscriber::EnvFilter;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

use crate::analysis::compute_word_cloud;
use crate::commit::{Commit, State, parse_from_file, write_to_file};
use crate::web::Action;

mod analysis;
mod commit;
mod web;

#[derive(clap::Parser)]
pub struct Args {
    commits_txt_path: PathBuf,

    /// update commit data from another commits.txt
    #[arg(long)]
    update_commit_data: Option<PathBuf>,

    /// sort by author, instead of the file’s original order
    #[arg(long)]
    sort_by_author: bool,

    /// sort by notes, instead of the file’s original order
    #[arg(long)]
    sort_by_notes: bool,

    /// sort by tags, instead of the file’s original order
    #[arg(long)]
    sort_by_tags: bool,

    /// filter commits to those containing this text
    #[arg(long)]
    filter_text_containing: Option<String>,

    /// start web server on this port
    #[arg(long)]
    web_server_port: Option<u16>,

    /// read `git show` outputs from this directory
    #[arg(long)]
    git_show_output_cache_path: Option<PathBuf>,

    /// read Highfive answers from this directory
    #[arg(long)]
    highfive_answers_path: Option<PathBuf>,

    /// compute word clouds with this dictionary file
    #[arg(long, default_value = "/usr/share/dict/words")]
    dictionary_path: PathBuf,
}

#[derive(Debug)]
pub struct App {
    path: PathBuf,
    filter_text_containing: Option<String>,

    commits: Vec<Commit>,
    index: usize,
    unroll: bool,
    exit: bool,
    edit_tag: bool,
    /// Current value of the input box
    input: String,
    /// Position of cursor in the editor area.
    byte_index: usize,
    cursor_pos: Cell<Option<(u16, u16)>>,
}

#[tokio::main]
async fn main() {
    // suppress rocket’s default logging to terminal.
    // you can also read the logs by changing `/dev/null` to some other path.
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer().with_writer(File::create("/dev/null").unwrap()))
        .with(if std::env::var("RUST_LOG").is_ok() {
            EnvFilter::builder().from_env_lossy()
        } else {
            "commit_triage=info,rocket=info".parse().unwrap()
        })
        .init();

    let args = &ARGS;
    let mut commits = parse_from_file(&args.commits_txt_path).unwrap();
    if args.sort_by_author {
        commits.sort_by(|p, q| p.authors.cmp(&q.authors));
    }
    if args.sort_by_notes {
        commits.sort_by(|p, q| p.notes().cmp(&q.notes()));
    }
    if args.sort_by_tags {
        commits.sort_by(|p, q| p.tags().cmp(q.tags()));
    }
    if let Some(other_commits_txt_path) = args.update_commit_data.as_ref() {
        let other_commits = parse_from_file(other_commits_txt_path).unwrap();
        write_to_file(&commits, &args.commits_txt_path, Some(&other_commits)).unwrap();
        return;
    }

    let web_server = std::thread::spawn(move || {
        if args.web_server_port.is_some() {
            crate::web::server()
        } else {
            Ok(())
        }
    });

    let mut app = App {
        index: 0,
        path: args.commits_txt_path.clone(),
        filter_text_containing: args.filter_text_containing.clone(),
        commits,
        edit_tag: false,
        unroll: false,
        exit: false,
        input: String::new(),
        byte_index: 0,
        cursor_pos: Cell::new(None),
    };

    // basically ratatui::run(), but that API is incompatible with async App::run(),
    // so we need to reimplement it here.
    let mut terminal = ratatui::init();
    app.run(&mut terminal).await.unwrap();
    ratatui::restore();

    if args.web_server_port.is_some() {
        eprintln!("shutting down...");
        crate::web::shutdown();
        web_server.join().unwrap().unwrap();
    }

    write_to_file(&app.commits, &app.path, None).unwrap();
}

impl App {
    /// runs the application's main loop until the user quits
    pub async fn run(&mut self, terminal: &mut DefaultTerminal) -> io::Result<()> {
        if let Some(index) = self
            .commits
            .iter()
            .position(|commit| commit.state == State::Untriaged && self.matches_filter(commit))
        {
            self.index = index;
        } else if let Some(index) = self
            .commits
            .iter()
            .position(|commit| self.matches_filter(commit))
        {
            self.index = index;
        }
        crate::web::update(&self.commits[self.index]);

        let mut terminal_events = EventStream::new();
        let mut web_actions = crate::web::ACTION.1.lock().await;
        while !self.exit {
            terminal.draw(|frame| self.draw(frame))?;
            tokio::select! {
                Some(Ok(event)) = terminal_events.next() => {
                    self.handle_terminal_event(terminal, event);
                },
                Some(action) = web_actions.recv() => {
                    match action {
                        Action::SetLabel(commits, label) => {
                            for commit in commits {
                                if let Some(commit) = self.commits.iter_mut().find(|other| other.hash_number == commit.hash_number) {
                                    commit.label = label.clone();
                                    crate::web::update(commit);
                                }
                            }
                            write_to_file(&self.commits, &self.path, None).unwrap();
                        },
                        Action::SetState(commits, state) => {
                            for commit in commits {
                                if let Some(commit) = self.commits.iter_mut().find(|other| other.hash_number == commit.hash_number) {
                                    commit.state = state;
                                    crate::web::update(commit);
                                }
                            }
                            write_to_file(&self.commits, &self.path, None).unwrap();
                        },
                        Action::GetCommits(tx) => {
                            tx.send(self.commits.clone()).unwrap();
                        },
                        Action::GetWordCloud(tx) => {
                            tx.send(compute_word_cloud(&self.commits)).unwrap();
                        },
                    }
                },
            }
        }
        Ok(())
    }

    fn draw(&self, frame: &mut Frame) {
        frame.render_widget(self, frame.area());
        if let Some((x, y)) = self.cursor_pos.take() {
            frame.set_cursor_position(Position::new(x, y));
        }
    }

    fn handle_terminal_event(&mut self, terminal: &mut DefaultTerminal, event: Event) {
        match event {
            // it's important to check that the event is a key press event as
            // crossterm also emits key release and repeat events on Windows.
            Event::Key(key_event) if key_event.kind == KeyEventKind::Press => {
                if key_event.code == KeyCode::Char('l')
                    && key_event.modifiers == KeyModifiers::CONTROL
                {
                    terminal.clear().unwrap();
                }
                self.handle_key_event(key_event)
            }
            _ => {}
        }
    }

    fn handle_key_event(&mut self, key_event: KeyEvent) {
        if !self.edit_tag {
            let has_shift = key_event.modifiers.contains(KeyModifiers::SHIFT);
            match key_event.code {
                KeyCode::Char('q') => self.exit(),
                KeyCode::Char('j') | KeyCode::Char('J') => self.next_index(has_shift),
                KeyCode::Char('k') | KeyCode::Char('K') => self.prev_index(has_shift),
                KeyCode::Char('o') => self.open_url(),
                KeyCode::Char('+') => self.update_state(State::Accepted),
                KeyCode::Char('-') => self.update_state(State::Ignored),
                KeyCode::Char('.') => self.update_state(State::Done),
                KeyCode::Char(' ') => self.unroll = !self.unroll,
                KeyCode::Char('t') => self.open_tag_editor(),
                _ => {}
            }
        } else {
            let has_ctrl = key_event.modifiers.contains(KeyModifiers::CONTROL);
            let has_alt = key_event.modifiers.contains(KeyModifiers::ALT);
            match key_event.code {
                KeyCode::Enter => self.commit_tag(true),
                KeyCode::Char(to_insert) => match (has_ctrl, has_alt, to_insert) {
                    (true, false, 'a') => self.byte_index = 0,
                    (true, false, 'e') => self.byte_index = self.input.len(),
                    (true, false, 'b') => self.move_cursor_left(false),
                    (false, true, 'b') => self.move_cursor_left(true),
                    (true, false, 'f') => self.move_cursor_right(false),
                    (false, true, 'f') => self.move_cursor_right(true),
                    (false, false, to_insert) => self.enter_char(to_insert),
                    _ => {}
                },
                KeyCode::Backspace => self.delete_char(true),
                KeyCode::Delete => self.delete_char(false),
                KeyCode::Left => self.move_cursor_left(has_ctrl),
                KeyCode::Right => self.move_cursor_right(has_ctrl),
                KeyCode::Home => self.byte_index = 0,
                KeyCode::End => self.byte_index = self.input.len(),
                KeyCode::Esc => self.commit_tag(false),
                _ => {}
            }
        }
    }

    fn move_cursor_left(&mut self, by_one_word: bool) {
        loop {
            self.byte_index = self
                .input
                .floor_char_boundary(self.byte_index.saturating_sub(1));
            self.byte_index = self.byte_index.clamp(0, self.input.len());
            if !by_one_word
                || self.byte_index == 0
                || self.input.as_bytes()[self.byte_index] == b' '
            {
                return;
            }
        }
    }

    fn move_cursor_right(&mut self, by_one_word: bool) {
        loop {
            self.byte_index = self
                .input
                .ceil_char_boundary(self.byte_index.saturating_add(1));
            self.byte_index = self.byte_index.clamp(0, self.input.len());
            if !by_one_word
                || self.byte_index == self.input.len()
                || self.input.as_bytes()[self.byte_index] == b' '
            {
                return;
            }
        }
    }

    fn enter_char(&mut self, new_char: char) {
        self.input.insert(self.byte_index, new_char);
        self.move_cursor_right(false);
    }

    fn delete_char(&mut self, backwards: bool) {
        if backwards {
            let is_not_cursor_leftmost = self.byte_index != 0;
            if is_not_cursor_leftmost {
                self.move_cursor_left(false);
                self.input.remove(self.byte_index);
            }
        } else {
            let is_not_cursor_rightmost = self.byte_index != self.input.len();
            if is_not_cursor_rightmost {
                self.input.remove(self.byte_index);
            }
        }
    }

    fn open_tag_editor(&mut self) {
        self.edit_tag = true;
        self.input = self.commits[self.index].label.clone();
        self.byte_index = self.input.len();
    }

    fn commit_tag(&mut self, commit: bool) {
        self.edit_tag = false;
        if commit {
            self.commits[self.index].label = std::mem::take(&mut self.input);
            write_to_file(&self.commits, &self.path, None).unwrap();
        }
    }

    fn matches_filter(&self, commit: &Commit) -> bool {
        if let Some(filter_text_containing) = self.filter_text_containing.as_ref()
            && !commit.filter_text().contains(filter_text_containing)
        {
            return false;
        }
        true
    }

    fn next_index(&mut self, shift: bool) {
        let original = self.index;
        loop {
            self.index += 1;
            if self.index >= self.commits.len() {
                self.index = 0;
            }
            if self.index == original {
                break;
            }
            if !self.matches_filter(&self.commits[self.index]) {
                continue;
            }
            if !shift {
                break;
            }
            if self.commits[self.index].state == State::Untriaged {
                break;
            }
        }
        crate::web::update(&self.commits[self.index]);
    }

    fn prev_index(&mut self, shift: bool) {
        let original = self.index;
        loop {
            if self.index == 0 {
                self.index = self.commits.len() - 1;
            } else {
                self.index -= 1;
            }
            if self.index == original {
                break;
            }
            if !self.matches_filter(&self.commits[self.index]) {
                continue;
            }
            if !shift {
                break;
            }
            if self.commits[self.index].state == State::Untriaged {
                break;
            }
        }
        crate::web::update(&self.commits[self.index]);
    }

    fn update_state(&mut self, state: State) {
        self.commits[self.index].state = state;
        self.next_index(true);
        write_to_file(&self.commits, &self.path, None).unwrap();
    }

    fn open_url(&self) {
        let child = Command::new("xdg-open")
            .arg(&self.commits[self.index].url)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .or_else(|_| {
                Command::new("open")
                    .arg(&self.commits[self.index].url)
                    .stdin(Stdio::null())
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .spawn()
            });
        let _ = child.unwrap().wait();
    }

    fn exit(&mut self) {
        self.exit = true;
    }
}

impl Widget for &App {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let remaining = self
            .commits
            .iter()
            .filter(|commit| commit.state == State::Untriaged)
            .count();
        let title = Line::from(
            format!(
                " Commit triage: {}/{} remaining",
                remaining,
                self.commits.len()
            )
            .bold(),
        );
        let instructions = Line::from(vec![
            " Next ".into(),
            "<J>".blue().bold(),
            " Previous ".into(),
            "<K>".blue().bold(),
            " Accept ".into(),
            "<+>".blue().bold(),
            " Ignore ".into(),
            "<->".blue().bold(),
            " Label ".into(),
            "<T>".blue().bold(),
            " Open ".into(),
            "<O>".blue().bold(),
            " Quit ".into(),
            "<Q> ".blue().bold(),
        ]);
        let block = Block::bordered()
            .padding(Padding::proportional(1))
            .title(title.centered())
            .title_bottom(instructions.centered())
            .border_set(border::THICK);

        let commit = &self.commits[self.index];
        let title = commit.title.clone();
        let title = match commit.state {
            State::Ignored => title.dark_gray(),
            State::Accepted => title.green(),
            State::Untriaged => title.yellow(),
            State::Done => title.reset(),
        };
        let mut lines = vec![
            title,
            commit.authors.join(", ").into(),
            commit.label.as_str().white(),
            "".into(),
        ];
        lines.extend(commit.hints.iter().map(|line| line.clone().cyan()));
        lines.push("".into());
        if self.unroll {
            lines.extend(commit.body.iter().map(|line| line.into()));
        } else {
            lines.push("<space> to show body".dark_gray());
        }
        lines.push("".into());
        lines.push(commit.date.split("T").next().unwrap().into());
        let commit_text = Text::from(lines.into_iter().map(Line::from).collect::<Vec<_>>());

        let input_area = block
            .inner(area)
            .centered(Constraint::Percentage(50), Constraint::Length(3));

        Paragraph::new(commit_text)
            .alignment(Alignment::Left)
            .block(block)
            .render(area, buf);

        if self.edit_tag {
            let input_block = Block::bordered()
                .title(Line::from("Edit label").centered())
                .border_set(border::THICK);

            Paragraph::new(self.input.as_str())
                .alignment(Alignment::Left)
                .block(input_block)
                .render(input_area, buf);

            self.cursor_pos.set(Some((
                // Draw the cursor at the current position in the input field.
                // This position is can be controlled via the left and right arrow key
                input_area.x + self.input[0..self.byte_index].chars().count() as u16 + 1,
                // Move one line down, from the border to the input line
                input_area.y + 1,
            )));
        }
    }
}
