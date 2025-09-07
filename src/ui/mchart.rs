use std::collections::HashMap;

use hdf5_metno::{Dataset, Selection};
use image::{DynamicImage, ImageBuffer, Rgb};
use plotters::{
    prelude::{BitMapBackend, IntoDrawingArea, WHITE},
    style::{Color as _, IntoFont, Palette},
};
use ratatui::{
    layout::Alignment,
    style::{Style, Stylize},
    text::Line,
    widgets::{Block, Borders, Paragraph},
};
use ratatui_image::{picker::Picker, protocol::StatefulProtocol, StatefulImage};

use crate::color_consts;

pub type Point = (f64, f64);
pub struct LineSerie {
    points: Vec<Point>,
    y_max: f64,
    y_min: f64,
    x_max: f64,
    x_min: f64,
}
pub type DatasetName = String;

pub struct MultiChartState {
    pub line_series: HashMap<DatasetName, LineSerie>,
    pub modified: bool,
    pub height: u32,
    pub width: u32,
    pub plot_buffer: Vec<u8>,
    pub picker: Picker,
    stateful_protocol: Option<StatefulProtocol>,
}

impl MultiChartState {
    pub fn new(picker: Picker) -> Self {
        Self {
            line_series: HashMap::new(),
            modified: false,
            height: 0,
            width: 0,
            plot_buffer: Vec::new(),
            picker,
            stateful_protocol: None,
        }
    }

    pub fn clear(&mut self) {
        self.line_series.clear();
        self.modified = true;
    }

    pub fn clear_ds(&mut self, dataset_name: &str) {
        if self.line_series.remove(dataset_name).is_some() {
            self.modified = true;
        }
    }

    pub fn add_linspace_series(&mut self, dataset: Dataset, selection: Selection) {
        if let Ok(data) = dataset.read_slice_1d::<f64, _>(selection) {
            let mut points: Vec<Point> = vec![];
            let mut y_max = f64::MIN;
            let mut y_min = f64::MAX;
            for (i, &y) in data.iter().enumerate() {
                let x = i as f64;
                points.push((x, y));
                if y > y_max {
                    y_max = y;
                }
                if y < y_min {
                    y_min = y;
                }
            }
            let points_len = points.len();
            let line_serie = LineSerie {
                points,
                y_max,
                y_min,
                x_min: 0.0,
                x_max: points_len as f64,
            };
            self.line_series
                .insert(dataset.name().to_string(), line_serie);
            self.modified = true;
        }
    }

    fn render_chart(&mut self) -> bool {
        if !self.modified {
            return false;
        }
        self.modified = false;

        let width = self.width;
        let height = self.height;
        self.plot_buffer = vec![0; (width * height * 3) as usize];
        let root =
            BitMapBackend::with_buffer(&mut self.plot_buffer, (width, height)).into_drawing_area();
        root.fill(&WHITE).unwrap();
        if self.line_series.is_empty() {
            return false;
        }
        let global_y_max = self
            .line_series
            .values()
            .map(|ls| ls.y_max)
            .fold(f64::MIN, f64::max);
        let global_y_min = self
            .line_series
            .values()
            .map(|ls| ls.y_min)
            .fold(f64::MAX, f64::min);
        let global_x_max = self
            .line_series
            .values()
            .map(|ls| ls.x_max)
            .fold(f64::MIN, f64::max);
        let global_x_min = self
            .line_series
            .values()
            .map(|ls| ls.x_min)
            .fold(f64::MAX, f64::min);

        let y_label_area_size = format!("{global_y_max:.4}").len() as u32 * 3 + 30;
        let chart = plotters::prelude::ChartBuilder::on(&root)
            .margin(10)
            .x_label_area_size(30)
            .y_label_area_size(y_label_area_size)
            .build_cartesian_2d(global_x_min..global_x_max, global_y_min..global_y_max);

        let mut chart = match chart {
            Ok(c) => c,
            Err(_) => return false,
        };
        // Draw the mesh (grid lines)
        chart
            .configure_mesh()
            .x_label_style(("sans-serif", 18).into_font())
            .y_label_style(("sans-serif", 18).into_font())
            .draw()
            .unwrap();

        // Add each line series but also add the name as legend
        for (i, (name, ls)) in self.line_series.iter().enumerate() {
            let color = plotters::prelude::Palette99::pick(i);
            let data = ls.points.iter().map(|(x, y)| (*x, *y));
            let line_series = plotters::prelude::LineSeries::new(data, &color);
            chart
                .draw_series(line_series)
                .unwrap()
                .label(name)
                .legend(move |(x, y)| {
                    plotters::prelude::PathElement::new(
                        vec![(x, y), (x + 20, y)],
                        plotters::prelude::ShapeStyle {
                            filled: true,
                            stroke_width: 2,
                            color: plotters::style::Color::to_rgba(&color),
                        },
                    )
                });
        }

        root.present().unwrap();

        true
    }

    pub(crate) fn render(&mut self, f: &mut ratatui::Frame<'_>) {
        let area = f.area();

        let header_block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(ratatui::style::Color::Green))
            .border_type(ratatui::widgets::BorderType::Rounded)
            .title("Multi-Chart".to_string())
            .bg(color_consts::BG_COLOR)
            .title_style(Style::default().fg(color_consts::TITLE).bold())
            .title_alignment(Alignment::Center);
        f.render_widget(header_block, area);
        let inner_area = ratatui::layout::Rect {
            x: area.x + 1,
            y: area.y + 1,
            width: area.width.saturating_sub(2),
            height: area.height.saturating_sub(2),
        };
        if self.line_series.is_empty() {
            self.render_empty(f, inner_area);
        } else {
            self.render_multi_chart(f, inner_area);
        }
    }

    fn render_empty(&mut self, f: &mut ratatui::Frame<'_>, area: ratatui::layout::Rect) {
        let no_data_message = "No data to plot.\nSelect datasets with 'm' to visualize them here.";
        let paragraph = ratatui::widgets::Paragraph::new(no_data_message)
            .alignment(Alignment::Center)
            .wrap(ratatui::widgets::Wrap { trim: true });
        f.render_widget(paragraph, area);
    }

    fn render_multi_chart(&mut self, f: &mut ratatui::Frame<'_>, area: ratatui::layout::Rect) {
        let series_len = self.line_series.len();
        let split = ratatui::layout::Layout::default()
            .direction(ratatui::layout::Direction::Vertical)
            .constraints(
                [
                    ratatui::layout::Constraint::Length(series_len as u16),
                    ratatui::layout::Constraint::Min(0),
                ]
                .as_ref(),
            )
            .split(area);
        let header_area = split[0];
        let chart_area = split[1];

        let mut legends: Vec<Line> = vec![];
        for (i, name) in self.line_series.keys().enumerate() {
            let color = plotters::prelude::Palette99::pick(i);
            let rgb = color.to_rgba();
            let colored_name = Line::from(format!("  ■ {name}\n"))
                .fg(ratatui::style::Color::Rgb(rgb.0, rgb.1, rgb.2))
                .bold();
            legends.push(colored_name);
        }
        let text = ratatui::text::Text::from(legends);

        f.render_widget(text, header_area);

        let (x, y) = self.picker.font_size();
        let new_height = chart_area.height as u32 * y as u32;
        let new_width = chart_area.width as u32 * x as u32;
        if new_height != self.height || new_width != self.width {
            self.height = new_height;
            self.width = new_width;
            self.modified = true;
            self.stateful_protocol = None; // Force re-creation of protocol
        }
        if self.render_chart() {
            let image = ImageBuffer::<Rgb<u8>, _>::from_raw(
                self.width,
                self.height,
                self.plot_buffer.clone(),
            )
            .expect("buffer size mismatch");
            let dyn_img = DynamicImage::ImageRgb8(image);
            let stateful_protocol = self.picker.new_resize_protocol(dyn_img);
            self.stateful_protocol = Some(stateful_protocol)
        };
        match self.stateful_protocol {
            None => {
                let no_data_message = "Rendering failed...?";
                let paragraph = ratatui::widgets::Paragraph::new(no_data_message)
                    .alignment(Alignment::Center)
                    .wrap(ratatui::widgets::Wrap { trim: true });
                f.render_widget(paragraph, chart_area);
            }
            Some(ref mut protocol) => {
                f.render_stateful_widget(StatefulImage::default(), chart_area, protocol);
            }
        }
    }
}
