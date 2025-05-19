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
    iter::zip,
    sync::{Arc, Mutex},
    time::Duration,
};

use color_eyre::Result;
use futures::{FutureExt as _, StreamExt as _};
use ratatui::{
    crossterm::event::{self, Event, KeyCode, KeyEventKind},
    layout::{Constraint, Layout, Rect},
    style::{self, Color, Modifier, Style, Stylize},
    text::Text,
    widgets::{Block, BorderType, Cell, HighlightSpacing, Paragraph, Row, Table, TableState},
    DefaultTerminal, Frame,
};
use style::palette::tailwind;
use tokio::select;

use futures_timer::Delay;

use crate::service::Bundle;

// const ITEM_HEIGHT: usize = 4;

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

pub struct App {
    state: TableState,
    // items: Vec<Data>,
    bundle: Arc<Mutex<Option<Bundle>>>,
    // bundle: Arc<Mutex<String>>,
    // longest_item_lens: (u16, u16, u16), // order is (name, address, email)
    // scroll_state: ScrollbarState,
    colors: TableColors,
}

impl App {
    pub fn new(bundle: Arc<Mutex<Option<Bundle>>>) -> Self {
        Self {
            state: TableState::default().with_selected(0),
            // longest_item_lens: constraint_len_calculator(&data_vec),
            // scroll_state: ScrollbarState::new((data_vec.len() - 1) * ITEM_HEIGHT),
            // items: data_vec,
            colors: TableColors::new(&tailwind::GRAY),
            bundle,
        }
    }
    pub fn next_row(&mut self, bundle: &Bundle) {
        let i = match self.state.selected() {
            Some(i) => {
                if i >= bundle.service_states.len() - 1 {
                    0
                } else {
                    i + 1
                }
            }
            None => 0,
        };
        self.state.select(Some(i));
        // self.scroll_state = self.scroll_state.position(i * ITEM_HEIGHT);
    }

    pub fn previous_row(&mut self, bundle: &Bundle) {
        let i = match self.state.selected() {
            Some(i) => {
                if i == 0 {
                    bundle.service_states.len() - 1
                } else {
                    i - 1
                }
            }
            None => 0,
        };
        self.state.select(Some(i));
        // self.scroll_state = self.scroll_state.position(i * ITEM_HEIGHT);
    }

    pub async fn run(mut self, mut terminal: DefaultTerminal) -> Result<()> {
        let mut reader = event::EventStream::new();

        loop {
            let bundle = {
                let b = self.bundle.lock().unwrap();
                b.clone()
            };
            terminal.draw(|frame| self.draw(frame, &bundle))?;

            let delay = Delay::new(Duration::from_millis(1_000)).fuse();
            let event = reader.next().fuse();

            select! {
                _ = delay => {  },
                maybe_event = event => {
                    match maybe_event {
                        Some(Ok(event)) => {
                            if let Event::Key(key) = event {
                                if key.kind == KeyEventKind::Press {

                                    match key.code {
                                        KeyCode::Char('q') | KeyCode::Esc => return Ok(()),
                                        KeyCode::Char('j') | KeyCode::Down if bundle.is_some() => self.next_row(&bundle.unwrap()),
                                        KeyCode::Char('k') | KeyCode::Up  if bundle.is_some() => self.previous_row(&bundle.unwrap()),
                                        _ => {}
                                    }
                                }
                            }
                        }
                        Some(Err(e)) => println!("Error: {:?}\r", e),
                        None => break,
                    }
                }
            };
        }
        Ok(())
    }

    fn draw(&mut self, frame: &mut Frame, bundle: &Option<Bundle>) {
        match bundle {
            None => {
                let p = Paragraph::new(format!(
                    "Connection failed {:?}",
                    std::time::SystemTime::now()
                ));
                frame.render_widget(p, frame.area());
            }
            Some(bundle) => {
                // self.reset_scrollbar(bundle.service_states.len());
                // if bundle.service_states.len() != self.scroll_state.content_length(content_length)
                let vertical = &Layout::vertical([Constraint::Min(5), Constraint::Length(4)]);
                let rects = vertical.split(frame.area());

                self.render_table(frame, rects[0], bundle);
                // self.render_scrollbar(frame, rects[0]);
                self.render_footer(frame, rects[1]);
            }
        }
    }

    fn render_table(&mut self, frame: &mut Frame, area: Rect, bundle: &Bundle) {
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

        let header = ["Fn name", "Interval", "State"]
            .into_iter()
            .map(Cell::from)
            .collect::<Row>()
            .style(header_style)
            .height(1);

        let services = zip(
            bundle.config.service_definitions.iter(),
            bundle.service_states.iter(),
        );

        let rows = services.enumerate().map(|(i, data)| {
            let color = match i % 2 {
                0 => self.colors.normal_row_color,
                _ => self.colors.alt_row_color,
            };
            let item = [
                &format!("{}", data.0.function_name),
                &format!("{}s", data.0.interval.as_secs()),
                &format!("{:?}", data.1),
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
            Constraint::from_fills([1, 1, 3]), /*  [
                                                   // + 1 is for padding.
                                                   Constraint::Length(self.longest_item_lens.0 + 1),
                                                   Constraint::Min(self.longest_item_lens.1 + 1),
                                                   Constraint::Min(self.longest_item_lens.2),
                                               ] */
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

    /* fn render_scrollbar(&mut self, frame: &mut Frame, area: Rect) {
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
    } */

    fn render_footer(&self, frame: &mut Frame, area: Rect) {
        const INFO_TEXT: [&str; 1] = ["(Esc) quit | (↑) move up | (↓) move down"];

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

    /* fn reset_scrollbar(&self, nb_services: usize) {
        if nb_services != self.scroll_state.content_length() {
            self.scroll_state = ScrollbarState::new(nb_services * ITEM_HEIGHT)
        }
    } */
}

/* fn constraint_len_calculator(items: &[Data]) -> (u16, u16, u16) {
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
} */
