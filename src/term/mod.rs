pub mod music_player;
pub mod playlist;
pub mod search;

use std::{
    collections::HashMap,
    io::{self, Stdout},
    time::{Duration, Instant},
};

use crossterm::{
    event::{
        self, DisableMouseCapture, EnableMouseCapture, Event, KeyEvent, KeyModifiers, MouseEvent,
    },
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use flume::Receiver;
use tui::{backend::CrosstermBackend, layout::Rect, Frame, Terminal};
use ytpapi::Video;

use crate::systems::logger::log;

use self::music_player::App;

pub trait Screen {
    fn name(&self) -> String;
    fn on_mouse_press(&mut self, mouse_event: MouseEvent, frame_data: &Rect) -> EventResponse;
    fn on_key_press(&mut self, mouse_event: KeyEvent, frame_data: &Rect) -> EventResponse;
    fn render(&mut self, frame: &mut Frame<CrosstermBackend<Stdout>>);
    fn handle_global_message(&mut self, message: ManagerMessage) -> EventResponse;
    fn close(&mut self, new_screen: String) -> EventResponse;
    fn open(&mut self) -> EventResponse;
}

#[derive(Debug, Clone)]
pub enum EventResponse {
    Message(Vec<ManagerMessage>),
    None,
}

#[derive(Debug, Clone)]
pub enum ManagerMessage {
    PassTo(String, Box<ManagerMessage>),
    ChangeState(String),
    UpdateApp(App),
    Quit,
    AddElementToChooser((String, Vec<Video>)),
}

pub struct Manager {
    screens: HashMap<String, Box<dyn Screen>>,
    current_screen: String,
}

impl Manager {
    pub fn new() -> Self {
        Manager {
            screens: HashMap::new(),
            current_screen: String::new(),
        }
    }
    pub fn add_screen(&mut self, screen: impl Screen + 'static) {
        self.screens.insert(screen.name(), Box::new(screen));
    }
    pub fn current_screen(&mut self) -> &mut Box<dyn Screen> {
        self.screens.get_mut(&self.current_screen).unwrap()
    }
    pub fn get_screen<'a>(&'a mut self, screen: &str) -> &'a mut Box<dyn Screen> {
        self.screens.get_mut(screen).unwrap()
    }
    pub fn set_current_screen(&mut self, screen: String) {
        self.current_screen = screen;
        let k = self.current_screen().open();
        self.handle_event(k);
    }
    pub fn handle_event(&mut self, event: EventResponse) -> bool {
        match event {
            EventResponse::Message(messages) => {
                for message in messages {
                    if self.handle_manager_message(message) {
                        return true;
                    }
                }
            }
            EventResponse::None => {}
        }
        false
    }
    pub fn handle_manager_message(&mut self, e: ManagerMessage) -> bool {
        match e {
            ManagerMessage::PassTo(e, a) => {
                self.get_screen(&e).handle_global_message(*a);
            }
            ManagerMessage::Quit => {
                self.current_screen().close(String::new());
                return true;
            }
            ManagerMessage::ChangeState(e) => {
                self.current_screen().close(e.clone());
                self.set_current_screen(e);
            }
            e => {
                log(format!(
                    "Unexpected message on manager (FORWARD it to a screen): {:?}",
                    e
                ));
            }
        }
        false
    }
    pub fn run(&mut self, updater: &Receiver<ManagerMessage>) -> Result<(), io::Error> {
        // setup terminal
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend)?;

        // create app and run it
        let tick_rate = Duration::from_millis(250);

        let mut last_tick = Instant::now();
        'a: loop {
            while let Ok(e) = updater.try_recv() {
                if self.handle_manager_message(e) {
                    break 'a;
                }
            }
            let rectsize = terminal.size()?;
            terminal.draw(|f| {
                self.current_screen().render(f);
            })?;

            let timeout = tick_rate
                .checked_sub(last_tick.elapsed())
                .unwrap_or_else(|| Duration::from_secs(0));
            if crossterm::event::poll(timeout)? {
                match event::read()? {
                    Event::Key(key) => {
                        if (key.code == event::KeyCode::Char('c')
                            || key.code == event::KeyCode::Char('d'))
                            && key.modifiers == KeyModifiers::CONTROL
                        {
                            break;
                        }
                        let k = self.current_screen().on_key_press(key, &rectsize);
                        if self.handle_event(k) {
                            break;
                        }
                    }
                    Event::Mouse(mouse) => {
                        let k = self.current_screen().on_mouse_press(mouse, &rectsize);
                        if self.handle_event(k) {
                            break;
                        }
                    }
                    _ => (),
                }
            }
            if last_tick.elapsed() >= tick_rate {
                last_tick = Instant::now();
            }
        }

        // restore terminal
        disable_raw_mode()?;
        execute!(
            terminal.backend_mut(),
            LeaveAlternateScreen,
            DisableMouseCapture
        )?;
        terminal.show_cursor()?;

        Ok(())
    }
}

pub fn split_y_start(f: Rect, start_size: u16) -> [Rect; 2] {
    let mut rectlistvol = f;
    rectlistvol.height = start_size;
    let mut rectprogress = f;
    rectprogress.y += start_size;
    rectprogress.height -= start_size * 2;
    [rectlistvol, rectprogress]
}
pub fn split_y(f: Rect, end_size: u16) -> [Rect; 2] {
    let mut rectlistvol = f;
    rectlistvol.height -= end_size;
    let mut rectprogress = f;
    rectprogress.y += rectprogress.height - end_size;
    rectprogress.height = end_size;
    [rectlistvol, rectprogress]
}
pub fn split_x(f: Rect, end_size: u16) -> [Rect; 2] {
    let mut rectlistvol = f;
    rectlistvol.width -= end_size;
    let mut rectprogress = f;
    rectprogress.x += rectprogress.width - end_size;
    rectprogress.width = end_size;
    [rectlistvol, rectprogress]
}

pub fn rect_contains(rect: &Rect, x: u16, y: u16, margin: u16) -> bool {
    rect.x + margin <= x
        && x <= rect.x + rect.width - margin
        && rect.y + margin <= y
        && y <= rect.y + rect.height - margin
}

pub fn relative_pos(rect: &Rect, x: u16, y: u16, margin: u16) -> (u16, u16) {
    (x - rect.x - margin, y - rect.y - margin)
}
