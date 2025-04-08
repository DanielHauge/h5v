use std::{cell::RefCell, rc::Rc};

use ratatui::{
    style::{Style, Styled},
    text::{Line, Span, Text},
};

use crate::{color_consts, h5f::H5FNode, ui::app::AppState};

#[derive(Debug)]
pub struct TreeItem<'a> {
    pub node: Rc<RefCell<H5FNode>>,
    pub line: Line<'a>,
    indent: usize,
}

impl<'a> AppState<'a> {
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
            indent: 0,
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

        let folder_icon = match (child.borrow().expanded, child.borrow().children.len() > 0) {
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
        let mut line_vec = prefix.iter().cloned().collect::<Vec<Span>>();
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
            indent: indent as usize,
        };
        tree_view.push(tree_item);
        let mut prefix_clone = prefix.clone();
        let mut indent = indent as u8;

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

#[cfg(test)]
mod tests {

    use crate::h5f::H5F;

    #[test]
    fn test_compute_tree_view_rec() {
        let h5f = H5F::open("example-femm-3d.h5".to_string()).unwrap();
        assert_eq!(h5f.root.borrow().expanded, true);
    }
}
