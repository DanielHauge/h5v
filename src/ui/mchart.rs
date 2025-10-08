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
    widgets::{Block, Borders},
};
use ratatui_image::{picker::Picker, protocol::StatefulProtocol, StatefulImage};

use crate::color_consts;

pub type Point = (f64, f64);
pub struct LineSerie {
    points: Vec<Point>,
    y_max: f64,
    y_min: f64,
    x_max: usize,
    x_min: usize,
}
pub type DatasetName = String;

pub struct MultiChartState {
    pub line_series: HashMap<DatasetName, LineSerie>,
    pub modified: bool,
    pub height: u32,
    pub width: u32,
    pub plot_buffer: Vec<u8>,
    pub picker: Picker,
    pub idx: usize,
    pub aoi_from: Option<usize>,
    pub aoi_to: Option<usize>,
    stateful_protocol: Option<StatefulProtocol>,
}

impl MultiChartState {
    pub fn new(picker: Picker) -> Self {
        Self {
            line_series: HashMap::new(),
            modified: false,
            idx: 0,
            height: 0,
            width: 0,
            plot_buffer: Vec::new(),
            picker,
            aoi_from: None,
            aoi_to: None,
            stateful_protocol: None,
        }
    }

    pub fn zoom_in(&mut self, percent: f64) {
        let max_x = self
            .line_series
            .values()
            .map(|ls| ls.x_max)
            .fold(usize::MIN, usize::max);
        let min_x = self
            .line_series
            .values()
            .map(|ls| ls.x_min)
            .fold(usize::MAX, usize::min);
        let actual_min = min_x.max(self.aoi_from.unwrap_or(min_x));
        let actual_max = max_x.min(self.aoi_to.unwrap_or(max_x));
        let range = actual_max - actual_min;
        let delta = range as f64 * percent / 100.0;
        let new_from = ((actual_min as f64 + delta).round() as usize).min(actual_max - 1);
        let new_to = ((actual_max as f64 - delta).round() as usize).max(actual_min + 1);
        self.aoi_from = Some(new_from);
        self.aoi_to = Some(new_to);
        self.modified = true;
    }

    pub fn clear_zoom(&mut self) {
        self.aoi_from = None;
        self.aoi_to = None;
        self.modified = true;
    }

    pub fn zoom_out(&mut self, percent: f64) {
        if self.aoi_from.is_none() && self.aoi_to.is_none() {
            return;
        }
        let max_x = self
            .line_series
            .values()
            .map(|ls| ls.x_max)
            .fold(usize::MIN, usize::max);
        let min_x = self
            .line_series
            .values()
            .map(|ls| ls.x_min)
            .fold(usize::MAX, usize::min);
        let actual_min = self.aoi_from.unwrap_or(min_x).max(min_x);
        let actual_max = self.aoi_to.unwrap_or(max_x).min(max_x);
        let range = actual_max - actual_min;
        let delta = range as f64 * percent / 100.0;
        let new_min = (actual_min as f64 - delta).round() as usize;
        let new_max = (actual_max as f64 + delta).round() as usize;
        if new_min <= min_x {
            self.aoi_from = None;
        } else {
            self.aoi_from = Some(new_min);
        }
        if new_max >= max_x {
            self.aoi_to = None;
        } else {
            self.aoi_to = Some(new_max);
        }
        self.modified = true;
    }

    pub fn pan_left(&mut self, percent: f64) {
        if self.aoi_from.is_none() && self.aoi_to.is_none() {
            return;
        }
        let max_x = self
            .line_series
            .values()
            .map(|ls| ls.x_max)
            .fold(usize::MIN, usize::max);
        let min_x = self
            .line_series
            .values()
            .map(|ls| ls.x_min)
            .fold(usize::MAX, usize::min);
        let actual_min = self.aoi_from.unwrap_or(min_x).max(min_x);
        let actual_max = self.aoi_to.unwrap_or(max_x).min(max_x);
        let range = actual_max - actual_min;
        let delta = (range as f64 * percent / 100.0).round() as usize;
        let new_min = actual_min.saturating_sub(delta);
        let new_max = actual_max.saturating_sub(delta);
        if new_min <= min_x {
            self.aoi_from = None;
        } else {
            self.aoi_from = Some(new_min);
        }
        if new_max >= max_x {
            self.aoi_to = None;
        } else {
            self.aoi_to = Some(new_max);
        }
        self.modified = true;
    }

    pub fn pan_right(&mut self, percent: f64) {
        if self.aoi_from.is_none() && self.aoi_to.is_none() {
            return;
        }
        let max_x = self
            .line_series
            .values()
            .map(|ls| ls.x_max)
            .fold(usize::MIN, usize::max);
        let min_x = self
            .line_series
            .values()
            .map(|ls| ls.x_min)
            .fold(usize::MAX, usize::min);
        let actual_min = self.aoi_from.unwrap_or(min_x).max(min_x);
        let actual_max = self.aoi_to.unwrap_or(max_x).min(max_x);
        let range = actual_max - actual_min;
        let delta = (range as f64 * percent / 100.0).round() as usize;
        let new_min = actual_min.saturating_add(delta);
        let new_max = actual_max.saturating_add(delta);
        if new_min <= min_x {
            self.aoi_from = None;
        } else {
            self.aoi_from = Some(new_min);
        }
        if new_max >= max_x {
            self.aoi_to = None;
        } else {
            self.aoi_to = Some(new_max);
        }
        self.modified = true;
    }

    // Zoom:
    // - up ( zoom in 5% )
    // - down ( zoom out 5% )
    // - left ( move left 5% )
    // - right ( move right 5% )

    pub fn clear_selected(&mut self) {
        self.line_series
            .iter()
            .nth(self.idx)
            .map(|(k, _)| k.clone())
            .map(|k| {
                self.line_series.remove(&k);
                self.modified = true;
            })
            .unwrap_or(());
        self.idx = self.idx.clamp(0, self.line_series.len().saturating_sub(1));
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
                x_min: 0,
                x_max: points_len,
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
        self.idx = self.idx.clamp(0, self.line_series.len().saturating_sub(1));
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

        let (x_min, x_max) = match (self.aoi_from, self.aoi_to) {
            (None, None) => {
                let global_x_max = self
                    .line_series
                    .values()
                    .map(|ls| ls.x_max)
                    .fold(usize::MIN, usize::max);
                let global_x_min = self
                    .line_series
                    .values()
                    .map(|ls| ls.x_min)
                    .fold(usize::MAX, usize::min);
                (global_x_min, global_x_max)
            }
            (Some(from), None) => {
                let global_x_max = self
                    .line_series
                    .values()
                    .map(|ls| ls.x_max)
                    .fold(usize::MIN, usize::max);
                let actual_from = from;
                (actual_from, global_x_max.max(actual_from))
            }
            (None, Some(to)) => {
                let global_x_min = self
                    .line_series
                    .values()
                    .map(|ls| ls.x_min)
                    .fold(usize::MAX, usize::min);
                let actual_to = to;
                (global_x_min.min(actual_to), actual_to)
            }
            (Some(from), Some(to)) => {
                let actual_from = from;
                let actual_to = to;
                if actual_from >= actual_to {
                    return false;
                }
                (actual_from, actual_to)
            }
        };

        let data_series = self.line_series.iter().map(|(name, ls)| {
            let local_x_min = ls.x_min.max(x_min).clamp(ls.x_min, ls.x_max);
            let local_x_max = ls.x_max.min(x_max).clamp(ls.x_min, ls.x_max);
            let data_points = ls.points[local_x_min..local_x_max]
                .iter()
                .map(|(x, y)| (*x, *y));
            (name.clone(), data_points)
        });

        let (y_max, y_min) = match (self.aoi_from, self.aoi_to) {
            (None, None) => {
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
                (global_y_max, global_y_min)
            }
            (Some(from), None) => {
                // Quikcly iter all y's in all series to find the global max and min
                let mut global_y_max = f64::MIN;
                let mut global_y_min = f64::MAX;
                for ls in self.line_series.values() {
                    if from >= ls.x_max {
                        continue;
                    }
                    let local_x_min = ls.x_min.max(from);
                    let local_x_max = ls.x_max.min(x_max);
                    for &(_, y) in &ls.points[local_x_min..local_x_max] {
                        if y > global_y_max {
                            global_y_max = y;
                        }
                        if y < global_y_min {
                            global_y_min = y;
                        }
                    }
                }
                (global_y_max, global_y_min)
            }
            (None, Some(to)) => {
                let mut global_y_max = f64::MIN;
                let mut global_y_min = f64::MAX;
                for ls in self.line_series.values() {
                    if to <= ls.x_min {
                        continue;
                    }
                    let x_min = ls.x_min.max(x_min);
                    let to = ls.x_max.min(to);
                    for &(_, y) in &ls.points[x_min..to] {
                        if y > global_y_max {
                            global_y_max = y;
                        }
                        if y < global_y_min {
                            global_y_min = y;
                        }
                    }
                }
                (global_y_max, global_y_min)
            }
            (Some(from), Some(to)) => {
                let mut global_y_max = f64::MIN;
                let mut global_y_min = f64::MAX;
                for ls in self.line_series.values() {
                    if to <= ls.x_min || from >= ls.x_max {
                        continue;
                    }
                    let from = ls.x_min.max(from);
                    let to = ls.x_max.min(to);
                    for &(_, y) in &ls.points[from..to] {
                        if y > global_y_max {
                            global_y_max = y;
                        }
                        if y < global_y_min {
                            global_y_min = y;
                        }
                    }
                }
                (global_y_max, global_y_min)
            }
        };

        let y_label_area_size = format!("{y_max:.4}").len() as u32 * 3 + 30;
        let chart = plotters::prelude::ChartBuilder::on(&root)
            .margin(10)
            .x_label_area_size(30)
            .y_label_area_size(y_label_area_size)
            .build_cartesian_2d(x_min as f64..x_max as f64, y_min..y_max);

        let mut chart = match chart {
            Ok(c) => c,
            Err(_) => return false,
        };
        chart
            .configure_mesh()
            .x_label_style(("sans-serif", 18).into_font())
            .y_label_style(("sans-serif", 18).into_font())
            .draw()
            .unwrap();

        for (i, (name, ls)) in data_series.enumerate() {
            let color = plotters::prelude::Palette99::pick(i);
            let data = ls;
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
            let colored_name = Line::from(format!(
                "{} ■ {name}\n",
                if i == self.idx { ">" } else { " " }
            ))
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
