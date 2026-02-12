use alacritty_terminal::{
    event::{Event as AlacEvent, EventListener, WindowSize},
    event_loop::{EventLoop, Msg, Notifier},
    grid::Dimensions,
    sync::FairMutex,
    term::{Config as TermConfig, Term},
    tty::{self, Options as PtyOptions, Shell},
};
use flume::{unbounded, Receiver, Sender};
use gpui::{px, Pixels};
use std::{collections::HashMap, env, path::Path, sync::Arc};

fn login_shell_args(shell_path: &str) -> Vec<String> {
    match Path::new(shell_path)
        .file_name()
        .and_then(|name| name.to_str())
    {
        Some("bash" | "zsh" | "fish") => vec!["-i".to_string(), "-l".to_string()],
        _ => Vec::new(),
    }
}

fn pty_env_overrides() -> HashMap<String, String> {
    let mut env_overrides = HashMap::new();
    let mut path_entries: Vec<String> = env::var("PATH")
        .unwrap_or_else(|_| "/usr/bin:/bin:/usr/sbin:/sbin".to_string())
        .split(':')
        .map(ToString::to_string)
        .collect();

    for extra in [
        "/opt/homebrew/bin",
        "/opt/homebrew/sbin",
        "/usr/local/bin",
        "/usr/local/sbin",
    ] {
        if !path_entries.iter().any(|entry| entry == extra) {
            path_entries.push(extra.to_string());
        }
    }

    env_overrides.insert("PATH".to_string(), path_entries.join(":"));
    env_overrides
}

/// Events sent from the terminal to the view
#[derive(Debug, Clone)]
pub enum TerminalEvent {
    /// Terminal content has changed, needs redraw
    Wakeup,
    /// Terminal title changed
    Title(String),
    /// Bell character received
    Bell,
    /// Terminal exited
    Exit,
}

/// Event listener that forwards alacritty events to our channel
#[derive(Clone)]
pub struct JsonEventListener(pub Sender<AlacEvent>);

impl EventListener for JsonEventListener {
    fn send_event(&self, event: AlacEvent) {
        let _ = self.0.send(event);
    }
}

/// Terminal dimensions in cells and pixels
#[derive(Debug, Clone, Copy)]
pub struct TerminalSize {
    pub cols: u16,
    pub rows: u16,
    pub cell_width: Pixels,
    pub cell_height: Pixels,
}

impl Default for TerminalSize {
    fn default() -> Self {
        Self {
            cols: 80,
            rows: 24,
            cell_width: px(9.0),
            cell_height: px(18.0),
        }
    }
}

impl From<TerminalSize> for WindowSize {
    fn from(size: TerminalSize) -> Self {
        // Extract the f32 value from Pixels
        let cell_width_f32: f32 = size.cell_width.into();
        let cell_height_f32: f32 = size.cell_height.into();
        WindowSize {
            num_cols: size.cols,
            num_lines: size.rows,
            cell_width: cell_width_f32 as u16,
            cell_height: cell_height_f32 as u16,
        }
    }
}

impl Dimensions for TerminalSize {
    fn total_lines(&self) -> usize {
        self.rows as usize
    }

    fn screen_lines(&self) -> usize {
        self.rows as usize
    }

    fn columns(&self) -> usize {
        self.cols as usize
    }

    fn last_column(&self) -> alacritty_terminal::index::Column {
        alacritty_terminal::index::Column(self.cols.saturating_sub(1) as usize)
    }

    fn bottommost_line(&self) -> alacritty_terminal::index::Line {
        alacritty_terminal::index::Line((self.rows as i32) - 1)
    }

    fn topmost_line(&self) -> alacritty_terminal::index::Line {
        alacritty_terminal::index::Line(0)
    }
}

/// The terminal state wrapper
pub struct Terminal {
    /// The alacritty terminal emulator
    term: Arc<FairMutex<Term<JsonEventListener>>>,
    /// Channel to send input to the PTY
    pty_tx: Notifier,
    /// Channel to receive events from alacritty
    events_rx: Receiver<AlacEvent>,
    /// Current terminal size
    size: TerminalSize,
}

impl Terminal {
    /// Create a new terminal with the given size
    pub fn new(size: TerminalSize) -> anyhow::Result<Self> {
        // Create event channels
        let (events_tx, events_rx) = unbounded();

        // Get shell from environment or default to bash
        let shell_path = env::var("SHELL").unwrap_or_else(|_| "/bin/bash".to_string());
        let shell = Shell::new(shell_path.clone(), login_shell_args(&shell_path));

        // Get working directory
        let working_directory = env::current_dir().ok();

        // Configure PTY
        let pty_options = PtyOptions {
            shell: Some(shell),
            working_directory,
            env: pty_env_overrides(),
            drain_on_exit: true,
        };

        // Create terminal config
        let term_config = TermConfig::default();

        // Create the terminal emulator
        let term = Term::new(term_config, &size, JsonEventListener(events_tx.clone()));
        let term = Arc::new(FairMutex::new(term));

        // Create PTY
        let window_id = 0;
        let pty = tty::new(&pty_options, size.into(), window_id)?;

        // Create and spawn the event loop
        let event_loop = EventLoop::new(
            term.clone(),
            JsonEventListener(events_tx),
            pty,
            false,
            false,
        )?;
        let pty_tx = Notifier(event_loop.channel());
        let _io_thread = event_loop.spawn();

        Ok(Self {
            term,
            pty_tx,
            events_rx,
            size,
        })
    }

    /// Write bytes to the PTY (user input)
    pub fn write(&self, input: &[u8]) {
        let _ = self.pty_tx.0.send(Msg::Input(input.to_vec().into()));
    }

    /// Write a string to the PTY
    pub fn write_str(&self, input: &str) {
        self.write(input.as_bytes());
    }

    /// Resize the terminal
    pub fn resize(&mut self, new_size: TerminalSize) {
        self.size = new_size;
        let _ = self.pty_tx.0.send(Msg::Resize(new_size.into()));
        self.term.lock().resize(new_size);
    }

    /// Get the current terminal size
    pub fn size(&self) -> TerminalSize {
        self.size
    }

    /// Process pending events and return true if terminal content changed
    pub fn process_events(&self) -> Vec<TerminalEvent> {
        let mut events = Vec::new();
        while let Ok(event) = self.events_rx.try_recv() {
            match event {
                AlacEvent::Wakeup => events.push(TerminalEvent::Wakeup),
                AlacEvent::Title(title) => events.push(TerminalEvent::Title(title)),
                AlacEvent::Bell => events.push(TerminalEvent::Bell),
                AlacEvent::Exit => events.push(TerminalEvent::Exit),
                _ => {}
            }
        }
        events
    }

    /// Access the terminal for reading cell content
    pub fn with_term<R>(&self, f: impl FnOnce(&Term<JsonEventListener>) -> R) -> R {
        let term = self.term.lock();
        f(&term)
    }

    /// Get the cursor position (column, row)
    pub fn cursor_position(&self) -> (usize, usize) {
        let term = self.term.lock();
        let cursor = term.grid().cursor.point;
        (cursor.column.0, cursor.line.0 as usize)
    }

    /// Check if there are pending events
    pub fn has_pending_events(&self) -> bool {
        !self.events_rx.is_empty()
    }
}

/// Convert a keystroke to terminal escape sequence
pub fn keystroke_to_input(key: &str, modifiers: gpui::Modifiers) -> Option<Vec<u8>> {
    // Handle special keys
    let input = match key {
        "enter" => Some(vec![b'\r']),
        "tab" => Some(vec![b'\t']),
        "escape" => Some(vec![0x1b]),
        "backspace" => Some(vec![0x7f]),
        "delete" => Some(b"\x1b[3~".to_vec()),
        "up" => Some(b"\x1b[A".to_vec()),
        "down" => Some(b"\x1b[B".to_vec()),
        "right" => Some(b"\x1b[C".to_vec()),
        "left" => Some(b"\x1b[D".to_vec()),
        "home" => Some(b"\x1b[H".to_vec()),
        "end" => Some(b"\x1b[F".to_vec()),
        "pageup" => Some(b"\x1b[5~".to_vec()),
        "pagedown" => Some(b"\x1b[6~".to_vec()),
        "space" => Some(vec![b' ']),
        _ => None,
    };

    if let Some(input) = input {
        return Some(input);
    }

    // Handle control key combinations
    if modifiers.control && key.len() == 1 {
        let c = key.chars().next().unwrap();
        if c.is_ascii_alphabetic() {
            // Ctrl+A = 0x01, Ctrl+B = 0x02, etc.
            let ctrl_char = (c.to_ascii_lowercase() as u8) - b'a' + 1;
            return Some(vec![ctrl_char]);
        }
    }

    // Handle regular characters
    if key.len() == 1 {
        let c = key.chars().next().unwrap();
        if c.is_ascii() {
            return Some(vec![c as u8]);
        } else {
            // UTF-8 encode non-ASCII characters
            let mut buf = [0u8; 4];
            let s = c.encode_utf8(&mut buf);
            return Some(s.as_bytes().to_vec());
        }
    }

    None
}
