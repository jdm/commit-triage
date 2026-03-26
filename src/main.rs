//use ratatui::{DefaultTerminal, Frame};
use std::cell::Cell;
use std::env;
use std::io;
use std::path::PathBuf;
use std::process::{Command, Stdio};

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::{
    DefaultTerminal, Frame,
    buffer::Buffer,
    layout::{Alignment, Constraint, Position, Rect},
    style::Stylize,
    symbols::border,
    text::{Line, Text},
    widgets::{Block, Padding, Paragraph, Widget},
};

use crate::commit::{Commit, State, parse_from_file, write_to_file};

mod commit;

#[derive(Debug)]
pub struct App {
    commits: Vec<Commit>,
    index: usize,
    unroll: bool,
    exit: bool,
    edit_tag: bool,
    /// Current value of the input box
    input: String,
    /// Position of cursor in the editor area.
    character_index: usize,
    cursor_pos: Cell<Option<(u16, u16)>>,
}

fn main() {
    let mut args = env::args();
    args.next();
    let path = PathBuf::from(args.next().unwrap());
    let commits = parse_from_file(&path).unwrap();
    let mut app = App {
        index: commits
            .iter()
            .position(|commit| commit.state == State::Untriaged)
            .unwrap_or(0),
        commits,
        edit_tag: false,
        unroll: false,
        exit: false,
        input: String::new(),
        character_index: 0,
        cursor_pos: Cell::new(None),
    };
    ratatui::run(|terminal| app.run(terminal)).unwrap();

    write_to_file(app.commits, &path).unwrap();
}

impl App {
    /// runs the application's main loop until the user quits
    pub fn run(&mut self, terminal: &mut DefaultTerminal) -> io::Result<()> {
        while !self.exit {
            terminal.draw(|frame| self.draw(frame))?;
            self.handle_events(terminal)?;
        }
        Ok(())
    }

    fn draw(&self, frame: &mut Frame) {
        frame.render_widget(self, frame.area());
        if let Some((x, y)) = self.cursor_pos.take() {
            frame.set_cursor_position(Position::new(x, y));
        }
    }

    fn handle_events(&mut self, terminal: &mut DefaultTerminal) -> io::Result<()> {
        match event::read()? {
            // it's important to check that the event is a key press event as
            // crossterm also emits key release and repeat events on Windows.
            Event::Key(key_event) if key_event.kind == KeyEventKind::Press => {
                self.handle_key_event(key_event, terminal)
            }
            _ => {}
        };
        Ok(())
    }

    fn handle_key_event(&mut self, key_event: KeyEvent, terminal: &mut DefaultTerminal) {
        if key_event.code == KeyCode::Char('l') && key_event.modifiers == KeyModifiers::CONTROL {
            terminal.clear().unwrap();
        }
        if !self.edit_tag {
            let has_shift = key_event.modifiers.contains(KeyModifiers::SHIFT);
            match key_event.code {
                KeyCode::Char('q') => self.exit(),
                KeyCode::Char('j') | KeyCode::Char('J') => self.next_index(has_shift),
                KeyCode::Char('k') | KeyCode::Char('K') => self.prev_index(has_shift),
                KeyCode::Char('o') => self.open_url(),
                KeyCode::Char('+') => self.update_state(State::Accepted),
                KeyCode::Char('-') => self.update_state(State::Ignored),
                KeyCode::Char(' ') => self.unroll = !self.unroll,
                KeyCode::Char('t') => self.open_tag_editor(),
                _ => {}
            }
        } else {
            match key_event.code {
                KeyCode::Enter => self.commit_tag(true),
                KeyCode::Char(to_insert) => self.enter_char(to_insert),
                KeyCode::Backspace => self.delete_char(),
                KeyCode::Left => self.move_cursor_left(),
                KeyCode::Right => self.move_cursor_right(),
                KeyCode::Esc => self.commit_tag(false),
                _ => {}
            }
        }
    }

    fn move_cursor_left(&mut self) {
        let cursor_moved_left = self.character_index.saturating_sub(1);
        self.character_index = self.clamp_cursor(cursor_moved_left);
    }

    fn move_cursor_right(&mut self) {
        let cursor_moved_right = self.character_index.saturating_add(1);
        self.character_index = self.clamp_cursor(cursor_moved_right);
    }

    fn enter_char(&mut self, new_char: char) {
        let index = self.byte_index();
        self.input.insert(index, new_char);
        self.move_cursor_right();
    }

    /// Returns the byte index based on the character position.
    ///
    /// Since each character in a string can be contain multiple bytes, it's necessary to calculate
    /// the byte index based on the index of the character.
    fn byte_index(&self) -> usize {
        self.input
            .char_indices()
            .map(|(i, _)| i)
            .nth(self.character_index)
            .unwrap_or(self.input.len())
    }

    fn delete_char(&mut self) {
        let is_not_cursor_leftmost = self.character_index != 0;
        if is_not_cursor_leftmost {
            // Method "remove" is not used on the saved text for deleting the selected char.
            // Reason: Using remove on String works on bytes instead of the chars.
            // Using remove would require special care because of char boundaries.

            let current_index = self.character_index;
            let from_left_to_current_index = current_index - 1;

            // Getting all characters before the selected character.
            let before_char_to_delete = self.input.chars().take(from_left_to_current_index);
            // Getting all characters after selected character.
            let after_char_to_delete = self.input.chars().skip(current_index);

            // Put all characters together except the selected one.
            // By leaving the selected one out, it is forgotten and therefore deleted.
            self.input = before_char_to_delete.chain(after_char_to_delete).collect();
            self.move_cursor_left();
        }
    }

    fn clamp_cursor(&self, new_cursor_pos: usize) -> usize {
        new_cursor_pos.clamp(0, self.input.chars().count())
    }

    fn open_tag_editor(&mut self) {
        self.edit_tag = true;
        self.input = self.commits[self.index].label.clone();
        self.character_index = self.input.chars().count();
    }

    fn commit_tag(&mut self, commit: bool) {
        self.edit_tag = false;
        if commit {
            self.commits[self.index].label = std::mem::take(&mut self.input);
        }
    }

    fn next_index(&mut self, shift: bool) {
        let original = self.index;
        loop {
            self.index += 1;
            if self.index >= self.commits.len() {
                self.index = 0;
            }
            if self.index == original || !shift {
                break;
            }
            if self.commits[self.index].state == State::Untriaged {
                break;
            }
        }
    }

    fn prev_index(&mut self, shift: bool) {
        let original = self.index;
        loop {
            if self.index == 0 {
                self.index = self.commits.len() - 1;
            } else {
                self.index -= 1;
            }
            if self.index == original || !shift {
                break;
            }
            if self.commits[self.index].state == State::Untriaged {
                break;
            }
        }
    }

    fn update_state(&mut self, state: State) {
        self.commits[self.index].state = state;
        self.next_index(false);
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
                input_area.x + self.character_index as u16 + 1,
                // Move one line down, from the border to the input line
                input_area.y + 1,
            )));
        }
    }
}
