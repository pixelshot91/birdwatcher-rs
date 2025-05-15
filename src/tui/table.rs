//! # [Ratatui] Table example
//!
//! The latest version of this example is available in the [examples] folder in the repository.
//!
//! Please note that the examples are designed to be run against the `main` branch of the Github
//! repository. This means that you may not be able to compile with the latest release version on
//! crates.io, or the one that you have installed locally.
//!
//! See the [examples readme] for more information on finding examples that match the version of the
//! library you are using.
//!
//! [Ratatui]: https://github.com/ratatui/ratatui
//! [examples]: https://github.com/ratatui/ratatui/blob/main/examples
//! [examples readme]: https://github.com/ratatui/ratatui/blob/main/examples/README.md

use std::{
    sync::{Arc, Mutex},
    time::{Duration, SystemTime},
};

use color_eyre::Result;
use crossterm::event::KeyModifiers;
use futures::{FutureExt as _, StreamExt as _};
use itertools::Itertools;
use ratatui::{
    crossterm::event::{self, Event, KeyCode, KeyEventKind},
    layout::{Constraint, Layout, Margin, Rect},
    style::{self, Color, Modifier, Style, Stylize},
    text::Text,
    widgets::{
        Block, BorderType, Cell, HighlightSpacing, Paragraph, Row, Scrollbar, ScrollbarOrientation,
        ScrollbarState, Table, TableState,
    },
    DefaultTerminal, Frame,
};
use style::palette::tailwind;
use tokio::select;
use unicode_width::UnicodeWidthStr;

use futures_timer::Delay;

use crate::service::Bundle;

const PALETTES: [tailwind::Palette; 4] = [
    tailwind::BLUE,
    tailwind::EMERALD,
    tailwind::INDIGO,
    tailwind::RED,
];
const INFO_TEXT: [&str; 2] = [
    "(Esc) quit | (↑) move up | (↓) move down | (←) move left | (→) move right",
    "(Shift + →) next color | (Shift + ←) previous color",
];

const ITEM_HEIGHT: usize = 4;

struct TableColors {
    buffer_bg: Color,
    header_bg: Color,
    header_fg: Color,
    row_fg: Color,
    selected_row_style_fg: Color,
    selected_column_style_fg: Color,
    selected_cell_style_fg: Color,
    normal_row_color: Color,
    alt_row_color: Color,
    footer_border_color: Color,
}

impl TableColors {
    const fn new(color: &tailwind::Palette) -> Self {
        Self {
            buffer_bg: tailwind::SLATE.c950,
            header_bg: color.c900,
            header_fg: tailwind::SLATE.c200,
            row_fg: tailwind::SLATE.c200,
            selected_row_style_fg: color.c400,
            selected_column_style_fg: color.c400,
            selected_cell_style_fg: color.c600,
            normal_row_color: tailwind::SLATE.c950,
            alt_row_color: tailwind::SLATE.c900,
            footer_border_color: color.c400,
        }
    }
}

struct Data {
    name: String,
    address: String,
    email: String,
}

impl Data {
    const fn ref_array(&self) -> [&String; 3] {
        [&self.name, &self.address, &self.email]
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn address(&self) -> &str {
        &self.address
    }

    fn email(&self) -> &str {
        &self.email
    }
}

pub struct App {
    state: TableState,
    // items: Vec<Data>,
    // bundle: Arc<Mutex<Option<Bundle>>>,
    bundle: Arc<Mutex<String>>,
    longest_item_lens: (u16, u16, u16), // order is (name, address, email)
    scroll_state: ScrollbarState,
    colors: TableColors,
    color_index: usize,
}

impl App {
    pub fn new(bundle: Arc<Mutex<String>>) -> Self {
        let data_vec = generate_fake_names();
        Self {
            state: TableState::default().with_selected(0),
            longest_item_lens: constraint_len_calculator(&data_vec),
            scroll_state: ScrollbarState::new((data_vec.len() - 1) * ITEM_HEIGHT),
            colors: TableColors::new(&PALETTES[0]),
            color_index: 0,
            // items: data_vec,
            bundle,
        }
    }
    pub fn next_row(&mut self) {
        // let i = match self.state.selected() {
        //     Some(i) => {
        //         if i >= self.items.len() - 1 {
        //             0
        //         } else {
        //             i + 1
        //         }
        //     }
        //     None => 0,
        // };
        // self.state.select(Some(i));
        // self.scroll_state = self.scroll_state.position(i * ITEM_HEIGHT);
    }

    pub fn previous_row(&mut self) {
        // let i = match self.state.selected() {
        //     Some(i) => {
        //         if i == 0 {
        //             self.items.len() - 1
        //         } else {
        //             i - 1
        //         }
        //     }
        //     None => 0,
        // };
        // self.state.select(Some(i));
        // self.scroll_state = self.scroll_state.position(i * ITEM_HEIGHT);
    }

    pub fn next_column(&mut self) {
        self.state.select_next_column();
    }

    pub fn previous_column(&mut self) {
        self.state.select_previous_column();
    }

    pub fn next_color(&mut self) {
        self.color_index = (self.color_index + 1) % PALETTES.len();
    }

    pub fn previous_color(&mut self) {
        let count = PALETTES.len();
        self.color_index = (self.color_index + count - 1) % count;
    }

    pub fn set_colors(&mut self) {
        self.colors = TableColors::new(&PALETTES[self.color_index]);
    }

    pub async fn run(mut self, mut terminal: DefaultTerminal) -> Result<()> {
        let mut reader = event::EventStream::new();

        loop {
            {
                // let s = self.bundle.lock().unwrap();
                // let s: String = s.clone();
                println!("{:?}", SystemTime::now());
                // println!("{:?}, bundle = {:?}", SystemTime::now(), s);
            }

            let mut delay = Delay::new(Duration::from_millis(1_000)).fuse();
            let mut event = reader.next().fuse();

            select! {
                _ = delay => { println!(".\r"); },
                maybe_event = event => {
                    match maybe_event {
                        Some(Ok(event)) => {
                            println!("Event::{:?}\r", event);

                            if let Event::Key(key) = event {
                                if key.kind == KeyEventKind::Press {
                                    match key.code {
                                        KeyCode::Char('q') | KeyCode::Esc => return Ok(()),
                                        _ => {}
                                    }
                                }
                            }
                            /*
                            if event == Event::Key(KeyCode::Char('c').into()) {
                                println!("Cursor position: {:?}\r", position());
                            }

                            if event == Event::Key(KeyCode::Esc.into()) {
                                break;
                            } */
                        }
                        Some(Err(e)) => println!("Error: {:?}\r", e),
                        None => break,
                    }
                }
            };

            /* if let Ok(true) = event::poll(Duration::from_millis(500)) {
                if let Event::Key(key) = event::read()? {
                    if key.kind == KeyEventKind::Press {
                        match key.code {
                            KeyCode::Char('q') | KeyCode::Esc => return Ok(()),
                            _ => {}
                        }
                    }
                }
            } */
        }
        Ok(())
        /* loop {
            terminal.draw(|frame| self.draw(frame))?;

            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    let shift_pressed = key.modifiers.contains(KeyModifiers::SHIFT);
                    match key.code {
                        KeyCode::Char('q') | KeyCode::Esc => return Ok(()),
                        KeyCode::Char('j') | KeyCode::Down => self.next_row(),
                        KeyCode::Char('k') | KeyCode::Up => self.previous_row(),
                        KeyCode::Char('l') | KeyCode::Right if shift_pressed => self.next_color(),
                        KeyCode::Char('h') | KeyCode::Left if shift_pressed => {
                            self.previous_color();
                        }
                        KeyCode::Char('l') | KeyCode::Right => self.next_column(),
                        KeyCode::Char('h') | KeyCode::Left => self.previous_column(),
                        _ => {}
                    }
                }
            }
        } */
    }

    fn draw(&mut self, frame: &mut Frame) {
        self.set_colors();

        let bundle = {
            let b = self.bundle.lock().unwrap();
            frame.render_widget(
                Paragraph::new(format!("dsa {:?} {:?}", b, SystemTime::now())),
                frame.area(),
            );
            b.clone()
        };
        return;
        /* match bundle {
            None => {
                let p = Paragraph::new(format!(
                    "Connection failed {:?}",
                    std::time::SystemTime::now()
                ));
                frame.render_widget(p, frame.area());
            }
            Some(bundle) => {
                let vertical = &Layout::vertical([Constraint::Min(5), Constraint::Length(4)]);
                let rects = vertical.split(frame.area());

                self.render_table(frame, rects[0], bundle);
                self.render_scrollbar(frame, rects[0]);
                self.render_footer(frame, rects[1]);
            }
        } */
    }

    fn render_table(&mut self, frame: &mut Frame, area: Rect, bundle: Bundle) {
        let header_style = Style::default()
            .fg(self.colors.header_fg)
            .bg(self.colors.header_bg);
        let selected_row_style = Style::default()
            .add_modifier(Modifier::REVERSED)
            .fg(self.colors.selected_row_style_fg);
        let selected_col_style = Style::default().fg(self.colors.selected_column_style_fg);
        let selected_cell_style = Style::default()
            .add_modifier(Modifier::REVERSED)
            .fg(self.colors.selected_cell_style_fg);

        let header = ["Name", "Address", "Email"]
            .into_iter()
            .map(Cell::from)
            .collect::<Row>()
            .style(header_style)
            .height(1);

        let rows = bundle.service_states.iter().enumerate().map(|(i, data)| {
            let color = match i % 2 {
                0 => self.colors.normal_row_color,
                _ => self.colors.alt_row_color,
            };
            let item = [
                &format!("{:?}", data),
                &format!("{:?}", data),
                &format!("{:?}", data),
            ];
            item.into_iter()
                .map(|content| Cell::from(Text::from(format!("\n{content}\n"))))
                .collect::<Row>()
                .style(Style::new().fg(self.colors.row_fg).bg(color))
                .height(4)
        });
        let bar = " █ ";
        let t = Table::new(
            rows,
            [
                // + 1 is for padding.
                Constraint::Length(self.longest_item_lens.0 + 1),
                Constraint::Min(self.longest_item_lens.1 + 1),
                Constraint::Min(self.longest_item_lens.2),
            ],
        )
        .header(header)
        .row_highlight_style(selected_row_style)
        .column_highlight_style(selected_col_style)
        .cell_highlight_style(selected_cell_style)
        .highlight_symbol(Text::from(vec![
            "".into(),
            bar.into(),
            bar.into(),
            "".into(),
        ]))
        .bg(self.colors.buffer_bg)
        .highlight_spacing(HighlightSpacing::Always);
        frame.render_stateful_widget(t, area, &mut self.state);
    }

    fn render_scrollbar(&mut self, frame: &mut Frame, area: Rect) {
        frame.render_stateful_widget(
            Scrollbar::default()
                .orientation(ScrollbarOrientation::VerticalRight)
                .begin_symbol(None)
                .end_symbol(None),
            area.inner(Margin {
                vertical: 1,
                horizontal: 1,
            }),
            &mut self.scroll_state,
        );
    }

    fn render_footer(&self, frame: &mut Frame, area: Rect) {
        let info_footer = Paragraph::new(Text::from_iter(INFO_TEXT))
            .style(
                Style::new()
                    .fg(self.colors.row_fg)
                    .bg(self.colors.buffer_bg),
            )
            .centered()
            .block(
                Block::bordered()
                    .border_type(BorderType::Double)
                    .border_style(Style::new().fg(self.colors.footer_border_color)),
            );
        frame.render_widget(info_footer, area);
    }
}

fn generate_fake_names() -> Vec<Data> {
    vec![Data {
        name: "MyName".to_string(),
        address: "add".to_string(),
        email: "email".to_string(),
    }]
}

fn constraint_len_calculator(items: &[Data]) -> (u16, u16, u16) {
    let name_len = items
        .iter()
        .map(Data::name)
        .map(UnicodeWidthStr::width)
        .max()
        .unwrap_or(0);
    let address_len = items
        .iter()
        .map(Data::address)
        .flat_map(str::lines)
        .map(UnicodeWidthStr::width)
        .max()
        .unwrap_or(0);
    let email_len = items
        .iter()
        .map(Data::email)
        .map(UnicodeWidthStr::width)
        .max()
        .unwrap_or(0);

    #[allow(clippy::cast_possible_truncation)]
    (name_len as u16, address_len as u16, email_len as u16)
}
