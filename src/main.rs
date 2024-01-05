use crossterm::event::{Event, KeyCode, KeyEvent};
use crossterm::terminal::ClearType;
use crossterm::{cursor, event, execute, queue, terminal};
use std::{cmp, env, fs, io};
use std::io::stdout;
use std::io::Write;
use std::path::Path;
use std::time::Duration;

enum Direction {
    Up,
    Down,
    Left,
    Right,
    TopScreen,
    BottomScreen,
    Home,
    End
}

struct CleanUp;

impl Drop for CleanUp {
    fn drop(&mut self) {        
        terminal::disable_raw_mode().expect("Could not turn off raw mode");
        Output::clear_screen().expect("Error");
    }
}

struct Reader;

impl Reader {
    fn read_key(&self) -> crossterm::Result<KeyEvent> {
        loop {
            if event::poll(Duration::from_millis(2000))? {
                if let Event::Key(event) = event::read()? {
                    return Ok(event);
                }
            }
        }
    }
}

struct EditorContents {
    content: String
}

impl EditorContents {
    fn new() -> Self {
        Self { content: String::new() }
    }

    fn push(&mut self, ch: char) {
        self.content.push(ch)
    }

    fn push_str(&mut self, s: &str) {
        self.content.push_str(s)
    }
}

impl io::Write for EditorContents {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        match std::str::from_utf8(buf) {
            Ok(s) => {
                self.content.push_str(s);
                Ok(s.len())
            },
            Err(_) => Err(io::ErrorKind::WriteZero.into()),
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        let out = write!(stdout(), "{}", self.content);
        stdout().flush()?;
        self.content.clear();
        out
    }
}

struct Editor {
    reader: Reader,
    output: Output,
}

impl Editor {
    fn new() -> Self {
        Self { reader: Reader, output: Output::new() }
    }

    fn ch_to_dir(ch: char) -> Direction {
        match ch {
            'h' => Direction::Left,
            'j' => Direction::Down,
            'k' => Direction::Up,
            'l' => Direction::Right,
            _ => unimplemented!()
        }
    }

    fn arrow_to_dir(key: KeyCode) -> Direction {
        match key {
            KeyCode::Up => Direction::Up,
            KeyCode::Down => Direction::Down,
            KeyCode::Left => Direction::Left,
            KeyCode::Right => Direction::Right,
            _ => unimplemented!()
        }
    }

    fn process_keypress(&mut self) -> crossterm::Result<bool> {
        match self.reader.read_key()? {
            KeyEvent {
                code: KeyCode::Char('q'),
                modifiers: event::KeyModifiers::CONTROL,
            } => return Ok(false),            
            KeyEvent {
                code: KeyCode::Char(val @ ('h' | 'j' | 'k' | 'l')),
                modifiers: event::KeyModifiers::NONE,
            } => self.output.move_cursor(Self::ch_to_dir(val)),            
            KeyEvent {
                code: dir @ (KeyCode::Up | KeyCode::Down | KeyCode::Left | KeyCode::Right),
                modifiers: event::KeyModifiers::NONE,
            } => self.output.move_cursor(Self::arrow_to_dir(dir)),
            KeyEvent {
                code: KeyCode::PageUp,
                modifiers: event::KeyModifiers::NONE
            } => self.output.move_cursor(Direction::TopScreen),
            KeyEvent {
                code: KeyCode::PageDown,
                modifiers: event::KeyModifiers::NONE
            } => self.output.move_cursor(Direction::BottomScreen),
            KeyEvent {
                code: KeyCode::Home,
                modifiers: event::KeyModifiers::NONE
            } => self.output.move_cursor(Direction::Home),
            KeyEvent {
                code: KeyCode::End,
                modifiers: event::KeyModifiers::NONE
            } => self.output.move_cursor(Direction::End),
            _ => {}
        }

        Ok(true)
    }

    fn run (&mut self) -> crossterm::Result<bool> {
        self.output.refresh_screen()?;
        self.process_keypress()
    }
}

struct Output {
    win_size: (usize, usize),
    editor_contents: EditorContents,
    cursor_controller: CursorController,
    editor_rows: EditorRows
}

impl Output {
    fn new() -> Self {
        let win_size = terminal::size().map(|(x, y)| (x as usize, y as usize))
                                       .unwrap();
        Self { 
            win_size,
            editor_contents: EditorContents::new(),
            cursor_controller: CursorController::new(win_size),
            editor_rows: EditorRows::new()
        }
    }

    fn clear_screen() -> crossterm::Result<()> {
        execute!(stdout(), terminal::Clear(ClearType::All))?;        
        execute!(stdout(), cursor::MoveTo(0, 0))
    }

    fn draw_rows(&mut self) {
        let screen_rows = self.win_size.1;
        let screen_cols = self.win_size.0;

        for j in 0..screen_rows {
            let buffer_row = j + self.cursor_controller.row_offset;
            if buffer_row >= self.editor_rows.number_of_rows() {
                if self.editor_rows.number_of_rows() == 0 && j == screen_rows / 3 {
                    let mut greeting = format!("Dana's Baby Editor {}", "0.0.1");
                    if greeting.len() > screen_cols {
                        greeting.truncate(screen_cols)
                    }

                    let mut padding = (screen_cols - greeting.len()) / 2;
                    if padding != 0 {
                        self.editor_contents.push('~');
                        padding -= 1;
                    }
                    (0..padding).for_each(|_| self.editor_contents.push(' '));
                    self.editor_contents.push_str(&greeting);
                }
                else {
                    self.editor_contents.push('~')
                }
            }
            else {
                let len = cmp::min(self.editor_rows.get_row(buffer_row).len(), screen_cols);
                self.editor_contents.push_str(&self.editor_rows.get_row(buffer_row)[..len])
            }

            queue!(
                self.editor_contents,
                terminal::Clear(ClearType::UntilNewLine)
            ).unwrap();

            if j < screen_rows - 1 {
                self.editor_contents.push_str("\r\n");
            }
        }
    }

    fn move_cursor(&mut self, dir: Direction) {
        self.cursor_controller.move_cursor(dir, self.editor_rows.number_of_rows());
    }

    fn refresh_screen(&mut self) -> crossterm::Result<()> {
        self.cursor_controller.scroll();
        queue!(self.editor_contents, cursor::Hide, cursor::MoveTo(0, 0))?;
        self.draw_rows();

        let cursor_x = self.cursor_controller.cursor_x;
        let cursor_y = self.cursor_controller.cursor_y - self.cursor_controller.row_offset;
        queue!(self.editor_contents, cursor::MoveTo(cursor_x as u16, cursor_y as u16), cursor::Show)?;
        self.editor_contents.flush()
    }
}

struct CursorController {
    cursor_x: usize,
    cursor_y: usize,
    screen_cols: usize,
    screen_rows: usize,
    row_offset: usize
}

impl CursorController {
    fn new(win_size: (usize, usize)) -> CursorController {
        Self { 
            cursor_x: 0, 
            cursor_y: 0, 
            screen_cols: 
            win_size.0, 
            screen_rows: win_size.1,
            row_offset: 0, 
        }
    }

    fn scroll(&mut self) {
        self.row_offset = cmp::min(self.row_offset, self.cursor_y);
        if self.cursor_y >= self.row_offset + self.screen_rows {
            //let mut msg = format!("cursor_y: {} screen_rows: {}", self.cursor_y, self.screen_rows);
            //panic!("{}", msg);
            self.row_offset = self.cursor_y - (self.screen_rows - 1);
        }
    }

    fn move_cursor(&mut self, dir: Direction, number_of_rows: usize) {
        match dir {
            Direction::Up => { self.cursor_y = self.cursor_y.saturating_sub(1) },
            Direction::Down => { 
                if self.cursor_y < number_of_rows {
                    self.cursor_y += 1;
                }
            },
            Direction::Left => { self.cursor_x = self.cursor_x.saturating_sub(1) },
            Direction::Right => { self.cursor_x = self.cursor_x.saturating_add(1) },
            Direction::TopScreen => { self.cursor_y = 0 },
            Direction::BottomScreen => { self.cursor_y = self.screen_rows - 1 }
            Direction::Home => { self.cursor_x = 0 },
            Direction::End => { self.cursor_x = self.screen_cols - 1 }
        }
    }
}

struct EditorRows {
    row_contents: Vec<Box<str>>
}

impl EditorRows {
    fn new() -> Self {        
        let mut arg = env::args();

        match arg.nth(1) {
            None => Self { row_contents: Vec::new() },            
            Some(file) => Self::from_file(file.as_ref()),
        }        
    }

    fn from_file(file: &Path) -> Self {
        let contents = fs::read_to_string(file).expect("Unable to read file");
        Self { row_contents: contents.lines().map(|txt| txt.into()).collect() }
    }

    fn number_of_rows(&self) -> usize {
        self.row_contents.len()
    }

    fn get_row(&self, n: usize) -> &str {
        &self.row_contents[n]
    }
}

fn main() -> crossterm::Result<()> {
    let _clean_up = CleanUp;
    
    terminal::enable_raw_mode()?;

    let mut editor = Editor::new();
    while editor.run()? {}

    Ok(())
}
