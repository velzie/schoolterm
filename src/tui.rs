use std::error::Error;

use console_engine::crossterm::event::{self, MouseEvent, MouseEventKind};
use console_engine::forms::FormField;
use console_engine::pixel::Pixel;
use console_engine::{
    crossterm::event::KeyEvent,
    events::Event,
    forms::{Form, FormOptions, FormStyle, HiddenText, Text},
    pixel,
    rect_style::BorderStyle,
    screen::Screen,
    Color, ConsoleEngine, KeyCode, KeyModifiers,
};
use termsize::Size;
use tokio::sync::{
    mpsc,
    oneshot::{Receiver, Sender},
};

use crate::{schooltool::Student, Command, Responder, UserData};

pub struct Tui {
    pub size: Size,
    pub engine: ConsoleEngine,
}
impl Tui {
    pub fn new() -> Result<Self, Box<dyn Error>> {
        let size = termsize::get().unwrap();
        let mut engine = ConsoleEngine::init(size.cols.into(), size.rows.into(), 20).unwrap();

        Ok(Tui { size, engine })
    }

    pub fn poll_blocking<T>(&mut self, mut resp_rx: Receiver<T>) -> Result<T, Box<dyn Error>> {
        let mut counter = 0;
        let mut dots = 0;
        loop {
            match self.engine.poll() {
                Event::Frame => {
                    match resp_rx.try_recv() {
                        Ok(student) => {
                            return Ok(student);
                        }
                        _ => (),
                    }
                    counter += 1;
                    if counter % 15 == 1 {
                        self.engine.clear_screen();
                        self.engine.fill(pixel::pxl_bg(
                            ' ',
                            Color::Rgb {
                                r: 10,
                                g: 10,
                                b: 10,
                            },
                        ));

                        self.engine.print_fbg(
                            self.size.cols as i32 / 2,
                            self.size.rows as i32 / 2,
                            &format!("Loading{}", ".".repeat(dots)),
                            Color::Black,
                            Color::Grey,
                        );
                        self.engine.draw();

                        dots += 1;
                        if dots > 5 {
                            dots = 0;
                        }
                    }
                }
                Event::Resize(x, y) => {
                    self.engine.resize(x.into(), y.into());
                    self.size = Size { rows: y, cols: x }
                }
                _ => (),
            }
        }
    }

    /// eventually we move this form into a widget, and then move tui_thread into here
    pub fn userdata_form(&mut self, userdata: &mut UserData) {
        let engine = &mut self.engine;

        let theme = FormStyle {
            border: Some(BorderStyle::new_light().with_colors(Color::DarkBlue, Color::Black)),
            ..Default::default()
        };
        // Create a new Form
        let mut form = Form::new(
            (self.size.cols / 2).into(),
            8,
            FormOptions {
                style: theme,
                label: Some("┤Login├"),
                ..Default::default()
            },
        );
        form.build_field::<Text>(
            "baseurl",
            FormOptions {
                style: theme,
                label: Some("SchoolTool Base URL: (usually ends with /schooltoolweb)"),
                ..Default::default()
            },
        );
        form.build_field::<Text>(
            "username",
            FormOptions {
                style: theme,
                label: Some("Username:"),
                ..Default::default()
            },
        );
        form.build_field::<HiddenText>(
            "password",
            FormOptions {
                style: theme,
                label: Some("Password:"),
                ..Default::default()
            },
        );
        form.set_active(true);

        while !form.is_finished() {
            // Poll next event
            match engine.poll() {
                // A frame has passed
                Event::Frame => {
                    engine.clear_screen();

                    engine.fill(pixel::pxl_bg(
                        ' ',
                        Color::Rgb {
                            r: 10,
                            g: 10,
                            b: 10,
                        },
                    ));
                    engine.print_screen(
                        (engine.get_width() / 4) as i32,
                        (engine.get_height() / 4) as i32,
                        form.draw((engine.frame_count % 8 > 3) as usize),
                    );
                    engine.draw();
                }
                Event::Resize(x, y) => {
                    engine.resize(x.into(), y.into());
                }

                // exit with Escape
                Event::Key(KeyEvent {
                    code: KeyCode::Esc,
                    modifiers: _,
                }) => {
                    break;
                }

                // exit with CTRL+C
                Event::Key(KeyEvent {
                    code: KeyCode::Char('c'),
                    modifiers: KeyModifiers::CONTROL,
                }) => {
                    panic!();
                    break;
                }
                // Let the form handle the unhandled events
                event => form.handle_event(event),
            }
        }
        userdata.baseurl = match form.get_field_output("baseurl").unwrap(){
            console_engine::forms::FormValue::String(s)=>s,
            _=>unreachable!()
        };
        userdata.username = match form.get_field_output("username").unwrap(){
            console_engine::forms::FormValue::String(s)=>s,
            _=>unreachable!()
        };

        userdata.password= match form.get_field_output("password").unwrap(){
            console_engine::forms::FormValue::String(s)=>s,
            _=>unreachable!()
        };
    }
}

#[derive(Debug, Clone)]
pub struct Theme {
    pub bg: Color,
    pub bg_accent: Color,
    pub fg: Color,
    pub fg_accent: Color,
    pub font: Color,
}

#[derive(Debug, Clone, Default)]
pub struct Rect {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
}

pub trait Widget: AsWidget {
    fn feed_event(&mut self, event: event::Event) -> Option<event::Event>;
    fn draw(&mut self, theme: &Theme, selected: bool) -> Screen;
    fn rect(&self) -> &Rect;
}
pub trait AsWidget {
    fn as_widget(&mut self) -> &mut dyn Widget;
}

impl<T: Widget> AsWidget for T {
    fn as_widget(&mut self) -> &mut dyn Widget {
        self
    }
}

pub struct TextDisplay {
    pub text: String,
    pub rect: Rect,
}
impl Widget for TextDisplay {
    fn feed_event(&mut self, event: event::Event) -> Option<event::Event> {
        Some(event)
    }
    fn draw(&mut self, theme: &Theme, selected: bool) -> Screen {
        let mut profile_screen = Screen::new(self.rect.w, self.rect.h);

        profile_screen.fill(pixel::pxl_bg(' ', theme.bg));
        profile_screen.rect_border(
            0,
            0,
            self.rect.w as i32 - 1,
            self.rect.h as i32 - 1,
            BorderStyle::new_light().with_colors(
                if selected { theme.fg_accent } else { theme.fg },
                theme.bg_accent,
            ),
        );

        profile_screen.print_fbg(2, 2, &self.text, theme.font, theme.bg);
        profile_screen
    }
    fn rect(&self) -> &Rect {
        &self.rect
    }
}

pub struct Table {
    pub indecies: Vec<String>,
    pub data: Vec<Vec<String>>,
    pub index: usize,
    pub rect: Rect,
}
impl Widget for Table {
    fn feed_event(&mut self, event: event::Event) -> Option<event::Event> {
        match event {
            event::Event::Key(KeyEvent {
                code: KeyCode::Down,
                modifiers: KeyModifiers::NONE,
            }) => {
                if self.index >= self.data.len() - 1 {
                    return Some(event);
                }
                self.index += 1;
            }

            event::Event::Key(KeyEvent {
                code: KeyCode::Up,
                modifiers: KeyModifiers::NONE,
            }) => {
                if self.index < 1 {
                    return Some(event);
                }
                self.index -= 1;
            }

            _ => return Some(event),
        }
        None
    }
    fn draw(&mut self, theme: &Theme, selected: bool) -> Screen {
        let mut screen = Screen::new(self.rect.w, self.rect.h);

        let borderfg = if selected { theme.fg_accent } else { theme.fg };
        screen.fill(pixel::pxl_bg(' ', theme.bg));
        screen.rect_border(
            0,
            0,
            self.rect.w as i32 - 1,
            self.rect.h as i32 - 1,
            BorderStyle::new_light().with_colors(borderfg, theme.bg_accent),
        );
        if self.data.len() <= 0 || self.indecies.len() <= 0 {
            return screen;
        }

        let mut averages = vec![0; self.data[0].len()];

        for row in &self.data {
            for (i, item) in row.iter().enumerate() {
                averages[i] += item.len();
            }
        }
        for coln in 0..averages.len() {
            averages[coln] /= self.data.len();
        }

        // let averages = self
        //     .data
        //     .iter()
        //     .map(|v| v.iter().map(|s| s.len()).reduce(|i, acc| i + acc).unwrap() / v.len());
        let mut lens: Vec<u32> = averages
            .clone()
            .into_iter()
            .map(|v| {
                (v as u32)
                    .max(3)
                    .min(self.rect.w / (self.indecies.len() as u32 + 1))
                    .clone()
            })
            .collect();
        let total_space = lens.clone().into_iter().reduce(|f, acc| f + acc).unwrap();
        let l = lens.len();
        lens[l - 1] = self.rect.w - total_space - 2;

        screen.h_line(
            0,
            2,
            self.rect.w as i32,
            pixel::pxl_fbg('═', borderfg, theme.bg_accent),
        );
        screen.print_fbg(0, 2, "╞", borderfg, theme.bg_accent);
        screen.print_fbg(self.rect.w as i32 - 1, 2, "╡", theme.fg, theme.bg_accent);
        let mut x = 2;
        for (i, s) in self.indecies.iter().enumerate() {
            screen.print_fbg(x as i32, 1, &s, theme.font, theme.bg);
            if i != 0 {
                screen.v_line(
                    x as i32 - 1,
                    1,
                    self.rect.h as i32 - 1,
                    pixel::pxl_fbg('│', borderfg, theme.bg_accent),
                );
                screen.print_fbg(x as i32 - 1, 2, "╪", borderfg, theme.bg_accent);

                screen.print_fbg(
                    x as i32 - 1,
                    self.rect.h as i32 - 1,
                    "┴",
                    borderfg,
                    theme.bg_accent,
                );

                screen.print_fbg(x as i32 - 1, 0, "┬", borderfg, theme.bg_accent);
            }
            x += lens[i];
        }
        let mut y = 3;
        for (i, row) in self.data.iter().enumerate() {
            let mut x = 2;
            for (j, s) in row.iter().enumerate() {
                let mut str = s.clone();
                str.truncate(lens[j] as usize - 1);
                screen.print_fbg(x as i32, y, &str, theme.font, theme.bg);
                x += lens[j];
            }
            y += 1;
        }

        screen
    }
    fn rect(&self) -> &Rect {
        &self.rect
    }
}

pub struct Drawer {
    pub buttons: Vec<String>,
    pub index: usize,
    pub clicked: bool,
    pub clicked_index: usize,
    pub rect: Rect,
}
impl Widget for Drawer {
    fn feed_event(&mut self, event: event::Event) -> Option<event::Event> {
        match event {
            event::Event::Key(KeyEvent {
                code: KeyCode::Right,
                modifiers: KeyModifiers::NONE,
            }) => {
                if self.index >= self.buttons.len() - 1 {
                    return Some(event);
                }
                self.index += 1;
            }

            event::Event::Key(KeyEvent {
                code: KeyCode::Left,
                modifiers: KeyModifiers::NONE,
            }) => {
                if self.index < 1 {
                    return Some(event);
                }
                self.index -= 1;
            }

            event::Event::Key(KeyEvent {
                code: KeyCode::Enter,
                modifiers: KeyModifiers::NONE,
            }) => {
                self.clicked_index = self.index;
                self.clicked = true;
                return None;
            }

            event::Event::Mouse(MouseEvent {
                kind: MouseEventKind::Down(b),
                column: col,
                row: row,
                modifiers: m,
            }) => {
                // dbg!(col as u32 /x_spacing);
                self.index += 1;
            }

            _ => return Some(event),
        }
        None
    }
    fn draw(&mut self, theme: &Theme, selected: bool) -> Screen {
        let mut profile_screen = Screen::new(self.rect.w, self.rect.h);

        profile_screen.fill(pixel::pxl_bg(' ', theme.bg));
        profile_screen.rect_border(
            0,
            0,
            self.rect.w as i32 - 1,
            self.rect.h as i32 - 1,
            BorderStyle::new_light().with_colors(
                if selected { theme.fg_accent } else { theme.fg },
                theme.bg_accent,
            ),
        );

        let total_text_space = if self.buttons.len() > 0 {
            self.buttons
                .iter()
                .map(|f| f.len() as u32)
                .reduce(|i, a| i + a)
                .unwrap()
        } else {
            0
        };

        let mut do_scrolling: bool = false;

        let x_spacing = if total_text_space + 2 >= self.rect.w {
            do_scrolling = true;
            2
        } else {
            (self.rect.w - total_text_space) / (self.buttons.len().max(1) as u32 + 1)
        };

        let y_spacing = self.rect.h as i32 / 2;

        let mut x = x_spacing;
        for (i, b) in self.buttons.iter_mut().enumerate() {
            profile_screen.print_fbg(
                x as i32,
                y_spacing,
                b,
                if self.index == i {
                    theme.bg_accent
                } else {
                    theme.font
                },
                if self.index == i {
                    theme.font
                } else {
                    theme.bg_accent
                },
            );
            x += b.len() as u32 + x_spacing;
        }

        // profile_screen.print_fbg(
        //     self.rect.w as i32 - 2,
        //     y_spacing,
        //     ">",
        //     theme.font,
        //     theme.bg_accent,
        // );
        profile_screen
    }
    fn rect(&self) -> &Rect {
        &self.rect
    }
}
