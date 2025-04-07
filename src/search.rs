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
        }
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
        let matches = self.tree.find(query_entry, 3);
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
    use crate::h5f::{H5FNodeRef, H5F};

    #[test]
    fn test_searcher_index() {
        let h5f = H5F::open("example-femm-3d.h5".to_string()).unwrap();
        h5f.index_recursive().unwrap();

        let root = h5f.root.borrow();
        let searcher = root.searcher.borrow();

        assert_eq!(searcher.lookup.len(), 11);
    }

    #[test]
    fn test_searcher_matches() {
        let h5f = H5F::open("example-femm-3d.h5".to_string()).unwrap();
        h5f.index_recursive().unwrap();

        let root = h5f.root.borrow();
        let searcher = root.searcher.borrow();

        let query = "mesh";
        let results = searcher.search(query);
        assert_eq!(results.len(), 1);
    }
}
