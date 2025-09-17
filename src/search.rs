use std::{collections::HashMap, fmt::Debug, ops::Range, sync::mpsc::Sender};

use bktree::{levenshtein_distance, BkTree};
use fuzzy_matcher::{skim::SkimMatcherV2, FuzzyMatcher};
use hdf5_metno::{File, Group};

use crate::h5f::{H5FNodeRef, HasPath};

type EntryKey = String;
type H5Path = String;

enum EntryValue {
    Path(H5Path),
    Query,
}

struct Entry {
    name: EntryKey,
    value: EntryValue,
}

impl AsRef<str> for Entry {
    fn as_ref(&self) -> &str {
        self.name.as_ref()
    }
}

pub struct Searcher {
    tree: BkTree<Entry>,
    lookup: HashMap<H5Path, H5FNodeRef>,
    pub query: String,
    pub line_cursor: usize,
    pub select_cursor: usize,
}

impl Debug for Searcher {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Searcher").finish()
    }
}

fn full_traversal(g: &Group) -> Vec<String> {
    g.iter_visit_default(vec![], |group, name, _, acc| match group.group(name) {
        Ok(g) => {
            acc.push(g.name());
            let grand_children = full_traversal(&g);
            acc.extend(grand_children);

            true
        }
        Err(_) => {
            let base_name = if group.name() == "/" {
                "/".to_string()
            } else {
                group.name() + "/"
            };
            acc.push(format!("{}{}", base_name, name));
            true
        }
    })
    .expect("Failed to get children")
}

fn index(file: File, result: Sender<()>) {
    let all_h5_paths = full_traversal(&file);
}

fn fuzzy_search<'a>(paths: &'a [String], query: &str) -> Vec<&'a str> {
    let matcher = SkimMatcherV2::default();

    let mut results: Vec<_> = paths
        .iter()
        .filter_map(|p| {
            matcher
                .fuzzy_match(p, query)
                .map(|score| (score, p.as_str()))
        })
        .collect();

    // sort best matches first
    results.sort_by(|a, b| b.0.cmp(&a.0));

    results.into_iter().map(|(_, path)| path).collect()
}

fn fuzzy_highlight(path: &str, query: &str) -> Vec<usize> {
    let matcher = SkimMatcherV2::default();
    if let Some((_, indices)) = matcher.fuzzy_indices(path, query) {
        indices
    } else {
        vec![]
    }
}

fn indices_to_spans(highlight_idx: &[usize]) -> Vec<Range<usize>> {
    let mut spans = vec![];
    if highlight_idx.is_empty() {
        return spans;
    }

    let mut start = highlight_idx[0];
    let mut end = highlight_idx[0] + 1;

    for &idx in &highlight_idx[1..] {
        if idx == end {
            end += 1;
        } else {
            spans.push(start..end);
            start = idx;
            end = idx + 1;
        }
    }
    spans.push(start..end);

    spans
}

impl Searcher {
    pub fn new() -> Self {
        Searcher {
            tree: BkTree::new(levenshtein_distance),
            lookup: HashMap::new(),
            query: String::new(),
            line_cursor: 0,
            select_cursor: 0,
        }
    }

    pub fn count_results(&self) -> usize {
        let results = self.search(&self.query);
        results.len()
    }

    pub fn add(&mut self, noderef: H5FNodeRef) {
        let path = noderef.node.borrow().node.path();
        let name = noderef.name.clone();
        let entry = Entry {
            name,
            value: EntryValue::Path(path.clone()),
        };
        self.tree.insert(entry);
        self.lookup.insert(path, noderef);
    }

    pub fn search(&self, query: &str) -> Vec<&H5FNodeRef> {
        let query_entry = Entry {
            name: query.to_string(),
            value: EntryValue::Query,
        };
        let mut matches = self.tree.find(query_entry, 8);
        matches.sort_by_key(|m| m.1);
        let mut results = vec![];
        for m in matches {
            let entry_value = &m.0.value;
            match entry_value {
                EntryValue::Path(path) => {
                    if let Some(noderef) = self.lookup.get(path) {
                        results.push(noderef);
                    } else {
                        // This case should not happen in a well-formed tree
                        // since we are searching for a query.
                        panic!("Unexpected path entry found in search results");
                    }
                }
                EntryValue::Query => {
                    // This case should not happen in a well-formed tree
                    // since we are searching for a query.
                    panic!("Unexpected query entry found in search results");
                }
            }
        }
        results
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_indexing() {
        let file = File::open("test.h5").unwrap();
        let all_h5_paths = full_traversal(&file);
        for path in all_h5_paths {
            eprintln!("{}", path);
        }
        panic!();
    }

    #[test]
    fn test_fuzzy_search() {
        let file = File::open("test.h5").unwrap();
        let all_h5_paths = full_traversal(&file);

        let query = "sins";
        let results = fuzzy_search(&all_h5_paths, query);
        for r in results.iter() {
            eprintln!("{}", r);
        }
        eprintln!("Total results: {}", results.len());
        panic!();
    }

    #[test]
    fn test_fuzzy_highlights() {
        let file = File::open("test.h5").unwrap();
        let all_h5_paths = full_traversal(&file);

        let query = "sins";
        let results = fuzzy_search(&all_h5_paths, query);
        for r in results.iter() {
            let highlight_idx = fuzzy_highlight(r, query);
            let highlight_spans = indices_to_spans(&highlight_idx);
            eprintln!("{query:?} {r:?} {highlight_idx:?} {highlight_spans:?}");
        }
        eprintln!("Total results: {}", results.len());
        panic!("This test only for quick ctx free exploration");
    }
}
