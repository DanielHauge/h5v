use std::{cell::RefCell, rc::Rc};

use ratatui::{
    layout::{Alignment, Margin, Offset, Rect},
    style::{Style, Stylize},
    text::{Line, Span, Text},
    widgets::{Block, Borders},
    Frame,
};

use crate::{
    configure,
    h5f::{H5FNode, HasPath},
    ui::{cursor::set_input_cursor, mchart::MultiChartState, std_comp_render::render_error},
};

use super::state::{AppState, Focus, Mode, TreeHitbox};

#[derive(Debug)]
pub struct TreeItem<'a> {
    pub node: Rc<RefCell<H5FNode>>,
    pub parent: Option<Rc<RefCell<H5FNode>>>,
    pub load_more: bool,
    pub line: Line<'a>,
}

#[derive(Clone)]
struct TreeSelection {
    node: Rc<RefCell<H5FNode>>,
    load_more: bool,
    ancestors: Vec<Rc<RefCell<H5FNode>>>,
}

fn tree_selection(treeview: &[TreeItem<'_>], cursor: usize) -> Option<TreeSelection> {
    let item = treeview.get(cursor)?;
    let mut ancestors = Vec::new();
    if item.load_more {
        ancestors.push(item.node.clone());
    }
    let mut parent = item.parent.clone();
    while let Some(node) = parent {
        parent = treeview
            .iter()
            .find(|item| !item.load_more && Rc::ptr_eq(&item.node, &node))
            .and_then(|item| item.parent.clone());
        ancestors.push(node);
    }
    Some(TreeSelection {
        node: item.node.clone(),
        load_more: item.load_more,
        ancestors,
    })
}

fn restored_tree_cursor(treeview: &[TreeItem<'_>], selection: &TreeSelection) -> Option<usize> {
    treeview
        .iter()
        .position(|item| {
            Rc::ptr_eq(&item.node, &selection.node) && item.load_more == selection.load_more
        })
        .or_else(|| {
            selection.ancestors.iter().find_map(|ancestor| {
                treeview
                    .iter()
                    .position(|item| !item.load_more && Rc::ptr_eq(&item.node, ancestor))
            })
        })
}

fn group_load_marker(load_error: bool) -> Option<&'static str> {
    if load_error {
        Some("[! retry]")
    } else {
        None
    }
}

impl AppState<'_> {
    pub fn compute_tree_view(&mut self) {
        let selection = tree_selection(&self.treeview, self.tree_view_cursor);
        let mut tree_view = Vec::new();
        let file_icon = Text::from(configure::configured_symbol(|symbols| {
            symbols.tree.root_file_icon
        }));
        let filenode = self.root.borrow().full_path();
        let text = Line::styled(
            format!("{} {}", file_icon, filenode),
            Style::default().fg(configure::themed_color(|colors| colors.tree.root_file)),
        );
        let root_tree_item = TreeItem {
            node: self.root.clone(),
            parent: None,
            load_more: false,
            line: text,
        };
        tree_view.push(root_tree_item);
        let children = compute_tree_view_rec(
            &self.root,
            vec![Span::raw("".to_string())],
            0,
            &self.multi_chart,
            None,
        );
        tree_view.extend(children);
        self.treeview = tree_view;
        self.tree_view_cursor = selection
            .as_ref()
            .and_then(|selection| restored_tree_cursor(&self.treeview, selection))
            .unwrap_or_else(|| {
                self.treeview
                    .len()
                    .saturating_sub(1)
                    .min(self.tree_view_cursor)
            });
    }
}

fn compute_tree_view_rec<'a>(
    node: &Rc<RefCell<H5FNode>>,
    prefix: Vec<Span<'a>>,
    indent: u8,
    mchart: &MultiChartState,
    parent: Option<Rc<RefCell<H5FNode>>>,
) -> Vec<TreeItem<'a>> {
    let mut tree_view = Vec::new();
    if !node.borrow().expanded {
        return tree_view;
    }

    let node_binding = node.borrow();

    let mut groups = node_binding.children.iter().peekable();
    let mut loading = 0;
    while let Some(child) = groups.next() {
        let c = child.borrow();
        let is_last_child = groups.peek().is_none();
        let connector = if is_last_child {
            configure::configured_symbol(|symbols| symbols.tree.connector_last)
        } else {
            configure::configured_symbol(|symbols| symbols.tree.connector_middle)
        };
        loading += 1;
        if loading > node_binding.view_loaded {
            // If we have more than 50 children, we stop rendering to avoid performance issues.
            // This is a simple way to handle large datasets.
            let adds = vec![
                Span::styled(
                    format!(
                        "{} ",
                        configure::configured_symbol(|symbols| symbols.tree.connector_last)
                    ),
                    Style::default().fg(configure::themed_color(|colors| colors.tree.load_more)),
                ),
                Span::styled(
                    "... ",
                    Style::default().fg(configure::themed_color(|colors| colors.tree.load_more)),
                ),
                Span::styled(
                    configure::configured_symbol(|symbols| symbols.tree.load_more_label),
                    Style::default().fg(configure::themed_color(|colors| colors.tree.load_more)),
                ),
            ];
            let mut spans = prefix.clone();
            spans.extend(adds);
            let line = Line::from(spans);
            let tree_item = TreeItem {
                node: node.clone(),
                parent: parent.clone(),
                load_more: true,
                line,
            };
            tree_view.push(tree_item);
            break;
        }
        let connector_span = Span::styled(
            connector,
            Style::default().fg(configure::themed_color(|colors| colors.tree.lines)),
        );
        let collapse_icon = if c.expanded {
            configure::configured_symbol(|symbols| symbols.tree.collapse_expanded)
        } else {
            configure::configured_symbol(|symbols| symbols.tree.collapse_collapsed)
        };

        let has_children = if c.read {
            !c.children.is_empty()
        } else if let crate::h5f::Node::Group(_, meta) = &c.node {
            meta.has_children
        } else {
            false
        };
        let folder_icon_base = if c.load_error.is_some() {
            "!"
        } else {
            match (c.expanded, has_children) {
                (true, true) => {
                    configure::configured_symbol(|symbols| symbols.tree.folder_open_branch)
                }
                (true, false) => {
                    configure::configured_symbol(|symbols| symbols.tree.folder_open_leaf)
                }
                (false, true) => {
                    configure::configured_symbol(|symbols| symbols.tree.folder_closed_branch)
                }
                (false, false) => {
                    configure::configured_symbol(|symbols| symbols.tree.folder_closed_leaf)
                }
            }
        };
        let folder_icon_link = c.icon();
        let folder_icon = format!("{}{}", folder_icon_base, folder_icon_link);

        let icon = match (c.is_group(), c.is_compound_container()) {
            (true, _) => folder_icon.to_string(),
            (false, true) => c.icon(),
            (false, false) => c.icon(),
        };

        let icon_color = match (
            child.borrow().is_group(),
            child.borrow().is_compound_container(),
            child.borrow().is_compound_leaf(),
        ) {
            (true, _, _) => configure::themed_color(|colors| colors.tree.group),
            (false, true, _) => configure::themed_color(|colors| colors.tree.compound),
            (false, _, true) => configure::themed_color(|colors| colors.tree.dataset_file),
            (false, false, false) => configure::themed_color(|colors| colors.tree.dataset_file),
        };

        let icon_span = Span::styled(icon, Style::default().fg(icon_color));
        let collapse_icon_span = match child.borrow().expanded {
            true => Span::styled(
                collapse_icon,
                Style::default().fg(configure::themed_color(|colors| colors.tree.file)),
            ),
            false => Span::styled(
                collapse_icon,
                Style::default().fg(configure::themed_color(|colors| colors.tree.lines)),
            ),
        };

        // let text = Text::from(format!("{}{} {} {}", prefix, connector, icon, child.name()));
        let mut line_vec = prefix.to_vec();
        line_vec.push(connector_span);
        line_vec.push(Span::raw(" "));
        if child.borrow().is_expandable() {
            line_vec.push(collapse_icon_span);
        }
        line_vec.push(icon_span);
        line_vec.push(Span::raw(" "));
        let name_color = match (
            child.borrow().is_group(),
            child.borrow().is_compound_container(),
            child.borrow().is_compound_leaf(),
        ) {
            (true, _, _) => configure::themed_color(|colors| colors.tree.group_name),
            (false, true, _) => configure::themed_color(|colors| colors.tree.compound_name),
            (false, _, true) => configure::themed_color(|colors| colors.tree.dataset),
            (false, false, false) => configure::themed_color(|colors| colors.tree.dataset),
        };
        line_vec.push(Span::styled(
            child.borrow().name(),
            Style::default().fg(name_color),
        ));
        if let Some(marker) = group_load_marker(c.load_error.is_some()) {
            line_vec.push(Span::styled(
                format!(" {marker}"),
                Style::default().fg(configure::themed_color(|colors| colors.text.error)),
            ));
        }
        let path = child.borrow().node.path();
        let memberships = mchart
            .chart_items()
            .iter()
            .enumerate()
            .filter(|(_, item)| item.matches_path(&path))
            .collect::<Vec<_>>();
        if !memberships.is_empty() {
            line_vec.push(Span::raw(" "));
            for (dot_idx, (_, item)) in memberships.iter().take(3).enumerate() {
                if dot_idx > 0 {
                    line_vec.push(Span::raw(""));
                }
                line_vec.push(Span::styled(
                    configure::configured_symbol(|symbols| symbols.chart.membership_marker),
                    Style::default().fg(configure::themed_color(|colors| {
                        colors.chart.series[item.color_slot % colors.chart.series.len()]
                    })),
                ));
            }
            if memberships.len() > 3 {
                line_vec.push(Span::styled(
                    format!("+{}", memberships.len() - 3),
                    Style::default().fg(configure::themed_color(|colors| {
                        colors.content.tree_membership_more
                    })),
                ));
            }
        }

        let line = Line::from(line_vec);

        let tree_item = TreeItem {
            node: child.clone(),
            parent: Some(node.clone()),
            load_more: false,
            line,
        };
        tree_view.push(tree_item);
        let mut prefix_clone = prefix.clone();
        let mut indent = indent;

        if is_last_child {
            indent += 3;
            prefix_clone.push(Span::raw("   "));
        } else {
            prefix_clone.push(
                Span::raw(configure::configured_symbol(|symbols| {
                    symbols.tree.vertical_guide
                }))
                .style(Style::default().fg(configure::themed_color(|colors| colors.tree.lines))),
            );
        };

        if child.borrow().is_expandable() {
            let children =
                compute_tree_view_rec(child, prefix_clone, indent, mchart, Some(node.clone()));
            tree_view.extend(children);
        }
    }

    tree_view
}

pub fn render_tree(f: &mut Frame, area: Rect, state: &mut AppState) {
    let outer_area = area;
    let bg = match (&state.focus, &state.mode) {
        (
            Focus::Tree(_),
            Mode::Normal
            | Mode::AttributeCreateDialog
            | Mode::AttributeDeleteDialog
            | Mode::FixedStringOverflowDialog
            | Mode::FixedStringResizeDialog,
        ) => configure::themed_color(|colors| colors.surface.focus_bg),
        _ => configure::themed_color(|colors| colors.surface.bg),
    };
    let header_block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(configure::themed_color(|colors| {
            colors.surface.panel_border
        })))
        .border_type(ratatui::widgets::BorderType::Rounded)
        .title(configure::configured_symbol(|symbols| symbols.title.tree).to_string())
        .bg(bg)
        .title_style(
            Style::default()
                .fg(configure::themed_color(|colors| colors.surface.panel_title))
                .bold(),
        )
        .title_alignment(Alignment::Center);
    f.render_widget(header_block, outer_area);

    let inner_area = outer_area.inner(Margin {
        horizontal: 2,
        vertical: 1,
    });

    let mut area = inner_area;
    let mode = state.mode.clone();
    match mode {
        Mode::Normal
        | Mode::Command
        | Mode::AttributeCreateDialog
        | Mode::AttributeDeleteDialog
        | Mode::FixedStringOverflowDialog
        | Mode::FixedStringResizeDialog => {
            let mut tree_view_skip_offset = 0;
            let mut highlight_index = state.tree_view_cursor;
            if area.height <= state.tree_view_cursor as u16 {
                let half = area.height / 2;
                tree_view_skip_offset = state.tree_view_cursor as u16 - half;
                highlight_index = half as usize;
            }

            state.ui_layout.tree = Some(TreeHitbox {
                outer: outer_area,
                inner: inner_area,
                row_offset: tree_view_skip_offset as usize,
                visible_rows: usize::min(
                    area.height as usize,
                    state
                        .treeview
                        .len()
                        .saturating_sub(tree_view_skip_offset as usize),
                ),
            });

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
                if highlight_index == i && matches!(state.focus, Focus::Tree(_)) {
                    f.render_widget(
                        text.bg(configure::themed_color(|colors| {
                            colors.surface.highlight_bg
                        })),
                        area,
                    );
                } else {
                    f.render_widget(text, area);
                }
                area = area.offset(Offset { x: 0, y: 1 });
            }
        }
        Mode::Search => {
            let Some(ref mut searcher) = state.searcher else {
                // render error
                render_error(f, &area, "Error: Searcher not initialized.".to_string());
                return;
            };
            let search_line_cursor = searcher.line_cursor;
            let search_query = searcher.query.clone();
            let search_results = searcher.search(&search_query);

            let mut view_offset = 0;

            let search_select_cursor = if searcher.select_cursor > search_results.len() {
                if search_results.is_empty() {
                    0
                } else {
                    search_results.len() - 1
                }
            } else {
                searcher.select_cursor
            };

            let mut highlight_index = searcher.select_cursor;
            if area.height <= search_select_cursor as u16 {
                let half = area.height / 2;
                view_offset = search_select_cursor as u16 - half;
                highlight_index = half as usize;
            }
            let results_count = search_results.len();

            // render search title with a search symbol:
            let search_icon_span = Span::styled(
                " ",
                Style::default().fg(configure::themed_color(|colors| colors.accent.search_icon)),
            );
            let search_text_span = Span::styled(
                format!(" {}", search_query),
                Style::default().fg(configure::themed_color(|colors| colors.text.search_text)),
            );
            let results_str = match results_count {
                0 => " (No results)".to_string(),
                1 => format!(" ({} result)", results_count),
                _ => format!(" ({} results)", results_count),
            };
            let search_count_span = Span::styled(
                results_str,
                Style::default().fg(configure::themed_color(|colors| colors.text.search_count)),
            );
            let search_line =
                Line::from(vec![search_icon_span, search_text_span, search_count_span]);
            f.render_widget(search_line, area);
            let mut area_pos = area.as_position();
            area_pos.x = area_pos.x + 3 + search_line_cursor as u16;
            set_input_cursor(f, area_pos);

            let mut offset = 1;
            let visible_rows = area.height.saturating_sub(1) as usize;
            for (i, result) in search_results.iter().skip(view_offset as usize).enumerate() {
                if i >= visible_rows {
                    break;
                }
                if i == highlight_index {
                    f.render_widget(
                        result
                            .clone()
                            .bg(configure::themed_color(|colors| {
                                colors.surface.highlight_bg
                            }))
                            .bold(),
                        area.offset(Offset { x: 3, y: offset }),
                    );
                } else {
                    f.render_widget(result.clone(), area.offset(Offset { x: 3, y: offset }));
                }
                offset += 1;
            }
        }
        Mode::Help | Mode::Logs | Mode::MultiChart => {}
    }
}

#[cfg(test)]
mod tests {
    use std::{cell::RefCell, rc::Rc};

    use ratatui::text::Line;

    use super::{group_load_marker, restored_tree_cursor, tree_selection, TreeItem};
    use crate::h5f::{H5FNode, Node};

    fn item(
        node: &Rc<RefCell<H5FNode>>,
        parent: Option<&Rc<RefCell<H5FNode>>>,
        load_more: bool,
    ) -> TreeItem<'static> {
        TreeItem {
            node: node.clone(),
            parent: parent.cloned(),
            load_more,
            line: Line::default(),
        }
    }

    #[test]
    fn recompute_preserves_the_same_visible_node() {
        let root = Rc::new(RefCell::new(H5FNode::new(Node::Broken("root".to_string()))));
        let child = Rc::new(RefCell::new(H5FNode::new(Node::Broken(
            "child".to_string(),
        ))));
        let before = vec![item(&root, None, false), item(&child, Some(&root), false)];
        let selection = tree_selection(&before, 1).expect("selection should exist");

        assert_eq!(restored_tree_cursor(&before, &selection), Some(1));
    }

    #[test]
    fn recompute_moves_a_hidden_selection_to_its_visible_parent() {
        let root = Rc::new(RefCell::new(H5FNode::new(Node::Broken("root".to_string()))));
        let parent = Rc::new(RefCell::new(H5FNode::new(Node::Broken(
            "parent".to_string(),
        ))));
        let child = Rc::new(RefCell::new(H5FNode::new(Node::Broken(
            "child".to_string(),
        ))));
        let before = vec![
            item(&root, None, false),
            item(&parent, Some(&root), false),
            item(&child, Some(&parent), false),
        ];
        let after = vec![item(&root, None, false), item(&parent, Some(&root), false)];
        let selection = tree_selection(&before, 2).expect("selection should exist");

        assert_eq!(restored_tree_cursor(&after, &selection), Some(1));
    }

    #[test]
    fn completed_load_more_keeps_selection_on_its_group() {
        let root = Rc::new(RefCell::new(H5FNode::new(Node::Broken("root".to_string()))));
        let group = Rc::new(RefCell::new(H5FNode::new(Node::Broken(
            "group".to_string(),
        ))));
        let before = vec![
            item(&root, None, false),
            item(&group, Some(&root), false),
            item(&group, Some(&root), true),
        ];
        let after = vec![item(&root, None, false), item(&group, Some(&root), false)];
        let selection = tree_selection(&before, 2).expect("selection should exist");

        assert_eq!(restored_tree_cursor(&after, &selection), Some(1));
    }

    #[test]
    fn failed_group_load_has_a_distinct_retry_marker() {
        assert_eq!(group_load_marker(false), None);
        assert_eq!(group_load_marker(true), Some("[! retry]"));
    }
}
