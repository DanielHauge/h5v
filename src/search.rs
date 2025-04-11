use std::{collections::HashMap, fmt::Debug};

use bktree::{levenshtein_distance, BkTree};

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
