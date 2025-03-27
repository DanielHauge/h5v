use ratatui::{
    style::{Color, Style},
    text::Text,
};

use crate::h5f::H5FNode;

#[derive(Debug)]
pub struct TreeItem<'a> {
    pub node: &'a H5FNode,
    pub text: Text<'a>,
}

pub fn compute_tree_view(root: &H5FNode) -> Vec<TreeItem> {
    let mut tree_view = Vec::new();
    let file_icon = Text::from("󰈚 ");
    let filenode = root.full_path();
    let text = Text::styled(
        format!("{} {}", file_icon, filenode),
        Style::default().fg(Color::Rgb(156, 210, 250)),
    );
    let root_tree_item = TreeItem { node: root, text };
    tree_view.push(root_tree_item);
    let children = compute_tree_view_rec(root, "".to_string());
    tree_view.extend(children);
    tree_view
}

pub fn compute_tree_view_rec(node: &H5FNode, prefix: String) -> Vec<TreeItem> {
    let mut tree_view = Vec::new();
    if !node.expanded {
        return tree_view;
    }
    let dataset_icon = "󰈚";
    let mut groups = node.children.iter().peekable();
    while let Some(child) = groups.next() {
        let is_last_child = groups.peek().is_none();
        let connector = if is_last_child { "└─" } else { "├─" };
        let folder_icon = if child.expanded { " " } else { " " };

        let icon = match child.is_group() {
            true => folder_icon,
            false => dataset_icon,
        };
        let text = Text::from(format!("{}{} {} {}", prefix, connector, icon, child.name()));

        let tree_item = TreeItem {
            node: child,
            text: Text::from(text),
        };
        tree_view.push(tree_item);

        let adjusted_prefix = if is_last_child {
            format!("{}   ", prefix)
        } else {
            format!("{}│  ", prefix)
        };

        if child.is_group() {
            let children = compute_tree_view_rec(child, adjusted_prefix);
            tree_view.extend(children);
        }
    }

    tree_view
}

#[cfg(test)]
mod tests {
    use crate::h5f::{H5FNode, H5F};

    fn expand_full_tree(node: &mut H5FNode) {
        node.expand().unwrap();
        for child in &mut node.children {
            expand_full_tree(child);
        }
    }

    #[test]
    fn test_compute_tree_view_rec() {
        let h5f = H5F::open("example-femm-3d.h5".to_string()).unwrap();
        assert_eq!(h5f.root.expanded, true);
    }

    #[test]
    fn test_compute_tree_view() {
        let mut h5f = H5F::open("example-femm-3d.h5".to_string()).unwrap();
        expand_full_tree(&mut h5f.root);
        let tree_view = super::compute_tree_view(&mut h5f.root);
        assert_eq!(tree_view.len(), 12);
        for item in tree_view {
            println!("{}", item.text.to_string());
        }
    }
}
