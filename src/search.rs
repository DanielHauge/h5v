use std::{fmt::Debug, ops::Range, vec};

use fuzzy_matcher::{skim::SkimMatcherV2, FuzzyMatcher};
use hdf5_metno::Group;
use ratatui::style::Style;
use ratatui::text::{Line, Span};

pub struct Searcher {
    paths: Vec<String>,
    pub query: String,
    pub line_cursor: usize,
    pub select_cursor: usize,
}

impl Debug for Searcher {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Searcher").finish()
    }
}

pub fn full_traversal(g: &Group) -> Vec<String> {
    let traversal_result =
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
        });
    traversal_result.unwrap_or_default()
}

pub(crate) fn fuzzy_match_score(candidate: &str, query: &str) -> Option<i64> {
    let matcher = SkimMatcherV2::default();
    matcher.fuzzy_match(candidate, query)
}

fn fuzzy_search<'a>(paths: &'a [String], query: &str) -> Vec<&'a str> {
    let mut results: Vec<_> = paths
        .iter()
        .filter_map(|p| fuzzy_match_score(p, query).map(|score| (score, p.as_str())))
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

pub(crate) fn indices_to_spans(highlight_idx: &[usize]) -> Vec<Range<usize>> {
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

fn char_spans_to_byte_spans(candidate: &str, spans: Vec<Range<usize>>) -> Vec<Range<usize>> {
    let char_starts = candidate
        .char_indices()
        .map(|(idx, _)| idx)
        .chain(std::iter::once(candidate.len()))
        .collect::<Vec<_>>();
    spans
        .into_iter()
        .filter_map(|span| {
            let start = *char_starts.get(span.start)?;
            let end = *char_starts.get(span.end)?;
            Some(start..end)
        })
        .collect()
}

pub(crate) fn fuzzy_highlight_spans(candidate: &str, query: &str) -> Vec<Range<usize>> {
    let highlight_idx = fuzzy_highlight(candidate, query);
    char_spans_to_byte_spans(candidate, indices_to_spans(&highlight_idx))
}

fn render_line_with_highlight<'a>(path: &'a str, query: &str) -> Line<'a> {
    let highlight_spans = fuzzy_highlight_spans(path, query);

    let mut spans = vec![];
    let mut last_end = 0;
    let base_style =
        Style::default().fg(crate::configure::themed_color(|colors| colors.text.primary));

    for span in highlight_spans {
        if span.start > last_end {
            spans.push(Span::styled(&path[last_end..span.start], base_style));
        }
        spans.push(Span::styled(
            &path[span.start..span.end],
            ratatui::style::Style::default().fg(crate::configure::themed_color(|colors| {
                colors.accent.search_highlight
            })),
        ));
        last_end = span.end;
    }

    if last_end < path.len() {
        spans.push(Span::styled(&path[last_end..], base_style));
    }

    Line::from(spans)
}

impl Searcher {
    pub fn new(paths: Vec<String>) -> Self {
        Searcher {
            paths,
            query: String::new(),
            line_cursor: 0,
            select_cursor: 0,
        }
    }

    pub fn count_results(&self) -> usize {
        self.result_paths(&self.query).len()
    }

    pub fn result_paths(&self, query: &str) -> Vec<&str> {
        fuzzy_search(&self.paths, query)
    }

    pub fn search(&self, query: &str) -> Vec<Line<'_>> {
        let rendered_lines = self
            .result_paths(query)
            .into_iter()
            .map(|p| render_line_with_highlight(p, query))
            .collect();
        rendered_lines
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
#[allow(clippy::expect_used)]
mod tests {
    use super::{fuzzy_highlight_spans, Searcher};

    #[test]
    fn returns_raw_result_paths() {
        let searcher = Searcher::new(vec![
            "/alpha".to_string(),
            "/group/dataset".to_string(),
            "/other".to_string(),
        ]);

        assert_eq!(searcher.result_paths("alph"), vec!["/alpha"]);
        assert_eq!(searcher.search("alph")[0].to_string(), "/alpha");
    }

    #[test]
    fn unicode_highlight_spans_use_byte_boundaries() {
        let spans = fuzzy_highlight_spans("/måling", "ml");
        assert_eq!(spans, vec![1..2, 4..5]);
    }

    #[test]
    fn search_render_handles_unicode_matches() {
        let searcher = Searcher::new(vec!["/måling".to_string()]);
        assert_eq!(searcher.search("ml")[0].to_string(), "/måling");
    }
}
