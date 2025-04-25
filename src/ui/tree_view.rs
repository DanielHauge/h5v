use std::{cell::RefCell, rc::Rc};

use ratatui::{
    layout::{Alignment, Margin, Offset, Rect},
    style::{Color, Style, Stylize},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};

use crate::{color_consts, h5f::H5FNode, ui::app::AppState};

use super::app::Mode;

#[derive(Debug)]
pub struct TreeItem<'a> {
    pub node: Rc<RefCell<H5FNode>>,
    pub line: Line<'a>,
}

impl AppState<'_> {
    pub fn compute_tree_view(&mut self) {
        let mut tree_view = Vec::new();
        let file_icon = Text::from("󰈚 ");
        let filenode = self.root.borrow().full_path();
        let text = Line::styled(
            format!("{} {}", file_icon, filenode),
            Style::default().fg(color_consts::ROOT_FILE_COLOR),
        );
        let root_tree_item = TreeItem {
            node: self.root.clone(),
            line: text,
        };
        tree_view.push(root_tree_item);
        let children = compute_tree_view_rec(&self.root, vec![Span::raw("".to_string())], 0);
        tree_view.extend(children);
        self.treeview = tree_view;
    }
}

fn compute_tree_view_rec<'a>(
    node: &Rc<RefCell<H5FNode>>,
    prefix: Vec<Span<'a>>,
    indent: u8,
) -> Vec<TreeItem<'a>> {
    let mut tree_view = Vec::new();
    if !node.borrow().expanded {
        return tree_view;
    }
    let dataset_icon = "󰈚 ";
    let node_binding = node.borrow_mut();
    let mut groups = node_binding.children.iter().peekable();
    while let Some(child) = groups.next() {
        let is_last_child = groups.peek().is_none();
        let connector = if is_last_child { "└─" } else { "├─" };
        let connector_span =
            Span::styled(connector, Style::default().fg(color_consts::LINES_COLOR));
        let collapse_icon = if child.borrow().expanded {
            " "
        } else {
            " "
        };

        // let folder_icon = if child.expanded { " " } else { " " };

        let folder_icon = match (child.borrow().expanded, !child.borrow().children.is_empty()) {
            (true, true) => " ",
            (true, false) => " ",
            (false, true) => " ",
            (false, false) => " ",
        };

        let icon = match child.borrow().is_group() {
            true => folder_icon,
            false => dataset_icon,
        };
        let icon_color = match child.borrow().is_group() {
            true => color_consts::GROUP_COLOR,
            false => color_consts::DATASET_FILE_COLOR,
        };
        let icon_span = Span::styled(icon, Style::default().fg(icon_color));
        let collapse_icon_span = match child.borrow().expanded {
            true => Span::styled(collapse_icon, Style::default().fg(color_consts::FILE_COLOR)),
            false => Span::styled(
                collapse_icon,
                Style::default().fg(color_consts::LINES_COLOR),
            ),
        };

        // let text = Text::from(format!("{}{} {} {}", prefix, connector, icon, child.name()));
        let mut line_vec = prefix.to_vec();
        line_vec.push(connector_span);
        line_vec.push(Span::raw(" "));
        if child.borrow().is_group() {
            line_vec.push(collapse_icon_span);
        }
        line_vec.push(icon_span);
        line_vec.push(Span::raw(" "));
        let name_color = match child.borrow().is_group() {
            true => color_consts::VARIABLE_BLUE,
            false => color_consts::DATASET_COLOR,
        };
        line_vec.push(Span::styled(
            child.borrow().name(),
            Style::default().fg(name_color),
        ));

        let line = Line::from(line_vec);

        let tree_item = TreeItem {
            node: child.clone(),
            line,
        };
        tree_view.push(tree_item);
        let mut prefix_clone = prefix.clone();
        let mut indent = indent;

        if is_last_child {
            indent += 3;
            prefix_clone.push(Span::raw("   "));
        } else {
            prefix_clone
                .push(Span::raw("│   ").style(Style::default().fg(color_consts::LINES_COLOR)));
        };

        if child.borrow().is_group() {
            let children = compute_tree_view_rec(child, prefix_clone, indent);
            tree_view.extend(children);
        }
    }

    tree_view
}

pub fn render_tree(f: &mut Frame, area: Rect, state: &mut AppState) {
    let header_block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Green))
        .border_type(ratatui::widgets::BorderType::Rounded)
        .title("Tree".to_string())
        .bg(color_consts::BG_COLOR)
        .title_style(Style::default().fg(Color::Yellow).bold())
        .title_alignment(Alignment::Center);
    f.render_widget(header_block, area);

    let inner_area = area.inner(Margin {
        horizontal: 2,
        vertical: 1,
    });

    let mut area = inner_area;
    let mode = state.mode.clone();
    match mode {
        Mode::Normal => {
            let mut tree_view_skip_offset = 0;
            let mut highlight_index = state.tree_view_cursor;
            if area.height <= state.tree_view_cursor as u16 {
                let half = area.height / 2;
                tree_view_skip_offset = state.tree_view_cursor as u16 - half;
                highlight_index = half as usize;
            }

            let treeview = &state.treeview;

            for (i, tree_item) in treeview
                .iter()
                .skip(tree_view_skip_offset as usize)
                .enumerate()
            {
                if i >= area.height as usize {
                    break;
                }
                let text = tree_item.line.clone();
                if highlight_index == i {
                    f.render_widget(text.bg(color_consts::HIGHLIGHT_BG_COLOR), area);
                } else {
                    f.render_widget(text, area);
                }
                area = area.offset(Offset { x: 0, y: 1 });
            }
        }
        Mode::Search => {
            if !state.indexed {
                match state.index() {
                    Ok(_) => {
                        state.indexed = true;
                    }
                    Err(_) => {
                        let error_text = Text::from("Error: Failed to index the file");
                        let error_paragraph = Paragraph::new(error_text)
                            .block(
                                Block::default()
                                    .borders(Borders::ALL)
                                    .border_style(Style::default().fg(Color::Red))
                                    .border_type(ratatui::widgets::BorderType::Rounded)
                                    .title("Error")
                                    .title_style(Style::default().fg(Color::Yellow).bold())
                                    .title_alignment(Alignment::Center),
                            )
                            .wrap(Wrap { trim: true });
                        f.render_widget(error_paragraph, area);
                        return;
                    }
                }
            }
            let root = state.root.borrow();
            let searcher = root.searcher.borrow();
            let search_line_cursor = searcher.line_cursor;
            let search_query = searcher.query.clone();
            let search_results = searcher.search(&search_query);

            let search_select_cursor = if searcher.select_cursor > search_results.len() {
                if search_results.is_empty() {
                    0
                } else {
                    search_results.len() - 1
                }
            } else {
                searcher.select_cursor
            };
            let results_count = search_results.len();

            // render search title with a search symbol:
            let search_icon_span = Span::styled(" ", Style::default().fg(Color::LightYellow));
            let search_text_span = Span::styled(
                format!(" {}", search_query),
                Style::default().fg(color_consts::SEARCH_TEXT_COLOR),
            );
            let results_str = match results_count {
                0 => " (No results)".to_string(),
                1 => format!(" ({} result)", results_count),
                _ => format!(" ({} results)", results_count),
            };
            let search_count_span = Span::styled(
                results_str,
                Style::default().fg(color_consts::SEARCH_COUNT_COLOR),
            );
            let search_line =
                Line::from(vec![search_icon_span, search_text_span, search_count_span]);
            f.render_widget(search_line, area);
            let mut area_pos = area.as_position();
            area_pos.x = area_pos.x + 3 + search_line_cursor as u16;
            f.set_cursor_position(area_pos);

            let mut offset = 1;
            for (i, result) in search_results.iter().enumerate() {
                if i >= area.height as usize {
                    break;
                }
                if i == search_select_cursor {
                    f.render_widget(
                        result
                            .rendered
                            .clone()
                            .bg(color_consts::HIGHLIGHT_BG_COLOR)
                            .bold(),
                        area.offset(Offset { x: 3, y: offset }),
                    );
                } else {
                    f.render_widget(
                        result.rendered.clone(),
                        area.offset(Offset { x: 3, y: offset }),
                    );
                }
                offset += 1;
            }
        }
        Mode::Help => unreachable!(),
    }
}

#[cfg(test)]
mod tests {

    use std::{cell::RefCell, rc::Rc};

    use crate::{h5f::H5F, search::Searcher};

    fn new_searcer() -> Rc<RefCell<Searcher>> {
        let searcher = Searcher::new();
        Rc::new(RefCell::new(searcher))
    }

    #[test]
    fn test_compute_tree_view_rec() {
        let h5f = H5F::open("test.h5".to_string(), new_searcer()).unwrap();
        assert!(h5f.root.borrow().expanded);
    }
}
