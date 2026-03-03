//! Define a TUI app that display the state of the services in a table.
//! It is used by `birdwatcher-cli` to display the state of the services.

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
    bundle: Arc<Mutex<Option<Bundle>>>,
    colors: TableColors,
}

impl App {
    pub fn new(bundle: Arc<Mutex<Option<Bundle>>>) -> Self {
        Self {
            state: TableState::default().with_selected(0),
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
    }

    /// Runs the TUI application.
    ///
    /// # Panics
    /// This function will panic if the mutex guarding the `bundle` is poisoned.
    pub async fn run(mut self, mut terminal: DefaultTerminal) -> Result<()> {
        let mut reader = event::EventStream::new();

        loop {
            let bundle = {
                let b = self.bundle.lock().unwrap();
                b.clone()
            };
            terminal.draw(|frame| self.draw(frame, bundle.as_ref()))?;

            let delay = Delay::new(Duration::from_secs(1)).fuse();
            let event = reader.next().fuse();

            select! {
                () = delay => {  },
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
                        Some(Err(e)) => println!("Error: {e:?}\r"),
                        None => break,
                    }
                }
            };
        }
        Ok(())
    }

    fn draw(&mut self, frame: &mut Frame, bundle: Option<&Bundle>) {
        match bundle {
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

        let rows = services
            .enumerate()
            .map(|(i, (service_definition, service_state))| {
                let color = match i % 2 {
                    0 => self.colors.normal_row_color,
                    _ => self.colors.alt_row_color,
                };
                let item = [
                    &service_definition.function_name.clone(),
                    &format!("{}s", service_definition.interval.as_secs()),
                    &format!("{service_state:?}"),
                ];
                item.into_iter()
                    .map(|content| Cell::from(Text::from(format!("\n{content}\n"))))
                    .collect::<Row>()
                    .style(Style::new().fg(self.colors.row_fg).bg(color))
                    .height(4)
            });
        let bar = " █ ";
        let table = Table::new(rows, Constraint::from_fills([1, 1, 3]))
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
        frame.render_stateful_widget(table, area, &mut self.state);
    }

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
}
