use std::ops::Range;

use hdf5_metno::File;

use crate::{
    configure,
    search::{full_traversal, fuzzy_highlight_spans, fuzzy_match_score},
};

use super::eval::{
    normalize_absolute_object_path, resolve_expression_scalar_value,
    resolve_expression_series_value,
};
use super::expression::{
    parse_expression_item_ref, parse_expression_scalar_ref, parse_expression_series_ref,
};
use super::*;

pub(super) const EXPRESSION_PROMPT_VISIBLE_SUGGESTIONS: usize = 4;

#[derive(Debug, Clone, PartialEq)]
pub(super) enum ExpressionPromptMode {
    New,
    EditExisting(ChartItemId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ExpressionPromptMessageKind {
    Error,
    Valid,
    Hint,
}

#[derive(Debug, Clone, PartialEq)]
pub(super) struct ExpressionPromptMessage {
    pub(super) kind: ExpressionPromptMessageKind,
    pub(super) text: String,
}

#[derive(Debug, Clone, PartialEq)]
pub(super) struct ExpressionPromptSuggestion {
    pub(super) symbol: String,
    pub(super) insert_text: String,
    pub(super) label: String,
    pub(super) detail: String,
    pub(super) kind: ExpressionPromptSuggestionKind,
    pub(super) highlight_spans: Vec<Range<usize>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ExpressionPromptSuggestionKind {
    ItemRef,
    Group,
    Dataset,
    CompoundLeaf,
    Attribute,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ExpressionPromptInputKind {
    Plain,
    ValidReference,
    InvalidReference,
}

#[derive(Debug, Clone, PartialEq)]
pub(super) struct ExpressionPromptInputSegment {
    pub(super) text: String,
    pub(super) kind: ExpressionPromptInputKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ExpressionAbsolutePathKind {
    Group,
    Dataset,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ExpressionAbsolutePathEntry {
    path: String,
    kind: ExpressionAbsolutePathKind,
    shape: Option<Vec<usize>>,
    detail: String,
}

#[derive(Debug, Clone, PartialEq)]
pub(super) struct ExpressionPromptState {
    pub(super) buffer: String,
    pub(super) cursor: usize,
    pub(super) mode: ExpressionPromptMode,
    pub(super) messages: Vec<ExpressionPromptMessage>,
    pub(super) suggestions: Vec<ExpressionPromptSuggestion>,
    pub(super) selected_suggestion: Option<usize>,
    pub(super) input_segments: Vec<ExpressionPromptInputSegment>,
}

impl ExpressionPromptState {
    pub(super) fn new(buffer: String, cursor: usize, mode: ExpressionPromptMode) -> Self {
        Self {
            buffer,
            cursor,
            mode,
            messages: Vec::new(),
            suggestions: Vec::new(),
            selected_suggestion: None,
            input_segments: Vec::new(),
        }
    }
}

impl MultiChartState {
    pub fn open_expression_prompt(&mut self) {
        let buffer = String::new();
        let cursor = buffer.len();
        self.expression_prompt = Some(ExpressionPromptState::new(
            buffer,
            cursor,
            ExpressionPromptMode::New,
        ));
        self.modified = true;
    }

    pub fn open_selected_item_for_edit(&mut self) -> Result<(), String> {
        let selected = self
            .selected_item()
            .ok_or_else(|| "No chart item selected".to_string())?;
        let buffer = selected.editable_expression().ok_or_else(|| {
            format!(
                "Selected series ${} cannot be edited as an expression",
                selected.id.0
            )
        })?;
        let cursor = buffer.len();
        self.expression_prompt = Some(ExpressionPromptState::new(
            buffer,
            cursor,
            ExpressionPromptMode::EditExisting(selected.id),
        ));
        self.modified = true;
        Ok(())
    }

    pub fn close_expression_prompt(&mut self) {
        self.expression_prompt = None;
        self.modified = true;
    }

    pub fn expression_insert_char(&mut self, ch: char) {
        if let Some(prompt) = self.expression_prompt.as_mut() {
            prompt.buffer.insert(prompt.cursor, ch);
            prompt.cursor += 1;
            prompt.selected_suggestion = None;
        }
    }

    pub fn expression_backspace(&mut self) {
        if let Some(prompt) = self.expression_prompt.as_mut() {
            if prompt.cursor > 0 {
                prompt.cursor -= 1;
                prompt.buffer.remove(prompt.cursor);
                prompt.selected_suggestion = None;
            }
        }
    }

    pub fn expression_delete(&mut self) {
        if let Some(prompt) = self.expression_prompt.as_mut() {
            if prompt.cursor < prompt.buffer.len() {
                prompt.buffer.remove(prompt.cursor);
                prompt.selected_suggestion = None;
            }
        }
    }

    pub fn expression_move_left(&mut self) {
        if let Some(prompt) = self.expression_prompt.as_mut() {
            if prompt.cursor > 0 {
                prompt.cursor -= 1;
            }
            prompt.selected_suggestion = None;
        }
    }

    pub fn expression_move_right(&mut self) {
        if let Some(prompt) = self.expression_prompt.as_mut() {
            if prompt.cursor < prompt.buffer.len() {
                prompt.cursor += 1;
            }
            prompt.selected_suggestion = None;
        }
    }

    pub fn expression_move_to_start(&mut self) {
        if let Some(prompt) = self.expression_prompt.as_mut() {
            prompt.cursor = 0;
            prompt.selected_suggestion = None;
        }
    }

    pub fn expression_move_to_end(&mut self) {
        if let Some(prompt) = self.expression_prompt.as_mut() {
            prompt.cursor = prompt.buffer.len();
            prompt.selected_suggestion = None;
        }
    }

    pub fn expression_clear(&mut self) {
        if let Some(prompt) = self.expression_prompt.as_mut() {
            prompt.buffer.clear();
            prompt.cursor = 0;
            prompt.selected_suggestion = None;
        }
    }

    pub fn expression_select_next_suggestion(&mut self) {
        if let Some(prompt) = self.expression_prompt.as_mut() {
            if !prompt.suggestions.is_empty() {
                let visible = prompt
                    .suggestions
                    .len()
                    .min(EXPRESSION_PROMPT_VISIBLE_SUGGESTIONS);
                prompt.selected_suggestion = Some(match prompt.selected_suggestion {
                    Some(selected) => (selected + 1) % visible,
                    None => 0,
                });
            }
        }
    }

    pub fn expression_select_prev_suggestion(&mut self) {
        if let Some(prompt) = self.expression_prompt.as_mut() {
            if !prompt.suggestions.is_empty() {
                let visible = prompt
                    .suggestions
                    .len()
                    .min(EXPRESSION_PROMPT_VISIBLE_SUGGESTIONS);
                prompt.selected_suggestion = Some(match prompt.selected_suggestion {
                    Some(0) | None => visible - 1,
                    Some(selected) => selected - 1,
                });
            }
        }
    }

    pub fn expression_deselect_suggestion(&mut self) -> bool {
        let Some(prompt) = self.expression_prompt.as_mut() else {
            return false;
        };
        prompt.selected_suggestion.take().is_some()
    }

    pub fn expression_has_selected_suggestion(&self) -> bool {
        self.expression_prompt
            .as_ref()
            .and_then(|prompt| prompt.selected_suggestion)
            .is_some()
    }

    pub fn expression_apply_selected_suggestion(&mut self) -> bool {
        let Some(prompt) = self.expression_prompt.as_mut() else {
            return false;
        };
        let Some((start, end, suggestion)) = current_expression_completion(prompt)
            .map(|(start, end, _, suggestion)| (start, end, suggestion.clone()))
        else {
            return false;
        };
        prompt
            .buffer
            .replace_range(start..end, &suggestion.insert_text);
        prompt.cursor = start + suggestion.insert_text.len();
        prompt.selected_suggestion = None;
        true
    }

    pub fn refresh_expression_prompt(&mut self, file: Option<&File>) {
        let Some((buffer, cursor, selected_suggestion)) =
            self.expression_prompt.as_ref().map(|prompt| {
                (
                    prompt.buffer.clone(),
                    prompt.cursor,
                    prompt.selected_suggestion,
                )
            })
        else {
            return;
        };
        let messages = expression_prompt_messages(self, file, &buffer);
        let suggestions = expression_prompt_suggestions(self, file, &buffer, cursor);
        let input_segments = expression_prompt_input_segments(self, file, &buffer);
        if let Some(prompt) = self.expression_prompt.as_mut() {
            prompt.messages = messages;
            prompt.suggestions = suggestions;
            prompt.selected_suggestion =
                selected_suggestion.filter(|selected| *selected < prompt.suggestions.len());
            prompt.input_segments = input_segments;
        }
    }

    pub fn submit_expression_prompt(&mut self, file: Option<&File>) -> Result<(), String> {
        let (expression, mode) = self
            .expression_prompt
            .as_ref()
            .map(|prompt| (prompt.buffer.trim().to_string(), prompt.mode.clone()))
            .ok_or_else(|| "Expression prompt is not active".to_string())?;
        if expression.is_empty() {
            self.set_expression_messages(vec![ExpressionPromptMessage {
                kind: ExpressionPromptMessageKind::Error,
                text: "Enter an expression before submitting".to_string(),
            }]);
            return Ok(());
        }

        let result = match mode {
            ExpressionPromptMode::New => self
                .create_expression_derived_with_file(expression.clone(), file)
                .map(|_| ()),
            ExpressionPromptMode::EditExisting(id) => {
                self.update_expression_item_with_file(id, expression.clone(), file)
            }
        };

        match result {
            Ok(_) => {
                self.close_expression_prompt();
                Ok(())
            }
            Err(error) => {
                self.set_expression_messages(vec![ExpressionPromptMessage {
                    kind: ExpressionPromptMessageKind::Error,
                    text: error,
                }]);
                Ok(())
            }
        }
    }

    pub(super) fn set_expression_messages(&mut self, messages: Vec<ExpressionPromptMessage>) {
        if let Some(prompt) = self.expression_prompt.as_mut() {
            prompt.messages = messages;
        }
    }
}

pub(super) fn expression_prompt_messages(
    state: &MultiChartState,
    file: Option<&File>,
    buffer: &str,
) -> Vec<ExpressionPromptMessage> {
    let trimmed = buffer.trim();
    if trimmed.is_empty() {
        return vec![
            ExpressionPromptMessage {
                kind: ExpressionPromptMessageKind::Hint,
                text: "Use $1, !/path[..,0], #/path:ATTR, or ($1, !/time[..])".to_string(),
            },
            ExpressionPromptMessage {
                kind: ExpressionPromptMessageKind::Hint,
                text: "Tab applies the selected suggestion.".to_string(),
            },
        ];
    }

    if expression_prompt_has_pending_completion(state, file, trimmed) {
        return Vec::new();
    }

    if MultiChartState::raw_dataset_reference(trimmed)
        .ok()
        .flatten()
        .is_some()
    {
        return vec![ExpressionPromptMessage {
            kind: ExpressionPromptMessageKind::Valid,
            text: "Dataset reference will load in the background when submitted".to_string(),
        }];
    }

    match state.evaluate_expression_with_file(trimmed, file) {
        Ok(evaluated) => {
            let result_kind = match evaluated.kind {
                DerivedExpressionKind::YSeries => "y-series",
                DerivedExpressionKind::XySeries => "x/y series",
            };
            vec![ExpressionPromptMessage {
                kind: ExpressionPromptMessageKind::Valid,
                text: format!(
                    "Valid {result_kind} with {} samples",
                    evaluated.points.len()
                ),
            }]
        }
        Err(error) => vec![ExpressionPromptMessage {
            kind: ExpressionPromptMessageKind::Error,
            text: error,
        }],
    }
}

fn expression_prompt_has_pending_completion(
    state: &MultiChartState,
    file: Option<&File>,
    buffer: &str,
) -> bool {
    let Some((_, end, fragment)) = current_expression_fragment(buffer, buffer.len()) else {
        return false;
    };
    if end != buffer.len() || fragment.is_empty() {
        return false;
    }
    if fragment.starts_with('$') {
        return state.items.iter().any(|item| {
            let candidate = format!("${}", item.id.0);
            candidate.starts_with(&fragment)
        });
    }
    if fragment.starts_with('!') || fragment.starts_with('#') {
        let Some(file) = file else {
            return false;
        };
        return !expression_path_suggestions(file, &fragment).is_empty();
    }
    false
}

pub(super) fn expression_prompt_input_segments(
    state: &MultiChartState,
    file: Option<&File>,
    buffer: &str,
) -> Vec<ExpressionPromptInputSegment> {
    let mut segments = Vec::new();
    let chars: Vec<(usize, char)> = buffer.char_indices().collect();
    let mut idx = 0;
    let mut plain_start = 0;

    while idx < chars.len() {
        let (start, ch) = chars[idx];
        if !matches!(ch, '$' | '!' | '#') {
            idx += 1;
            continue;
        }
        let end = consume_expression_reference_fragment(buffer, &chars, idx);
        if end <= start + ch.len_utf8() {
            idx += 1;
            continue;
        }
        if plain_start < start {
            segments.push(ExpressionPromptInputSegment {
                text: buffer[plain_start..start].to_string(),
                kind: ExpressionPromptInputKind::Plain,
            });
        }
        let fragment = &buffer[start..end];
        let kind = if end == buffer.len() {
            ExpressionPromptInputKind::Plain
        } else {
            match validate_expression_reference_fragment(state, file, fragment) {
                Ok(()) => ExpressionPromptInputKind::ValidReference,
                Err(_) => ExpressionPromptInputKind::InvalidReference,
            }
        };
        segments.push(ExpressionPromptInputSegment {
            text: fragment.to_string(),
            kind,
        });
        plain_start = end;
        while idx < chars.len() && chars[idx].0 < end {
            idx += 1;
        }
    }

    if plain_start < buffer.len() {
        segments.push(ExpressionPromptInputSegment {
            text: buffer[plain_start..].to_string(),
            kind: ExpressionPromptInputKind::Plain,
        });
    }

    if segments.is_empty() {
        segments.push(ExpressionPromptInputSegment {
            text: buffer.to_string(),
            kind: ExpressionPromptInputKind::Plain,
        });
    }
    segments
}

pub(super) fn consume_expression_reference_fragment(
    buffer: &str,
    chars: &[(usize, char)],
    start_idx: usize,
) -> usize {
    let start_char = chars[start_idx].1;
    let mut cursor = start_idx + 1;
    let mut bracket_depth = 0usize;
    while cursor < chars.len() {
        let ch = chars[cursor].1;
        match ch {
            '[' => bracket_depth += 1,
            ']' => bracket_depth = bracket_depth.saturating_sub(1),
            _ => {}
        }
        let is_delimiter = ch.is_whitespace()
            || (bracket_depth == 0 && matches!(ch, '+' | '-' | '*' | ',' | '(' | ')'))
            || (start_char == '$' && ch == '/');
        if is_delimiter {
            break;
        }
        cursor += 1;
    }
    chars
        .get(cursor)
        .map(|(offset, _)| *offset)
        .unwrap_or(buffer.len())
}

fn validate_expression_reference_fragment(
    state: &MultiChartState,
    file: Option<&File>,
    fragment: &str,
) -> Result<(), String> {
    match fragment.chars().next() {
        Some('$') => {
            let mut chars = fragment[1..].chars().peekable();
            let item_ref = parse_expression_item_ref(&mut chars)?;
            if chars.next().is_some() {
                return Err(format!("Invalid chart item reference {fragment}"));
            }
            let _ = resolve_expression_item_value(
                state,
                &item_ref,
                super::ExpressionSeriesResolution::Overview,
            )?;
            Ok(())
        }
        Some('!') => {
            let mut chars = fragment[1..].chars().peekable();
            let series_ref = parse_expression_series_ref(&mut chars)?;
            if chars.next().is_some() {
                return Err(format!("Invalid series reference {fragment}"));
            }
            let file = file.ok_or_else(|| "No file loaded for series references".to_string())?;
            let _ = resolve_expression_series_value(
                state,
                file,
                &series_ref,
                super::ExpressionSeriesResolution::Overview,
                true,
            )?;
            Ok(())
        }
        Some('#') => {
            let mut chars = fragment[1..].chars().peekable();
            let scalar_ref = parse_expression_scalar_ref(&mut chars)?;
            if chars.next().is_some() {
                return Err(format!("Invalid scalar reference {fragment}"));
            }
            let file = file.ok_or_else(|| "No file loaded for scalar references".to_string())?;
            let _ = resolve_expression_scalar_value(state, file, &scalar_ref)?;
            Ok(())
        }
        _ => Ok(()),
    }
}

pub(super) fn current_expression_completion(
    prompt: &ExpressionPromptState,
) -> Option<(usize, usize, String, &ExpressionPromptSuggestion)> {
    let (start, end, fragment) = current_expression_fragment(&prompt.buffer, prompt.cursor)?;
    let suggestion = prompt
        .selected_suggestion
        .and_then(|selected| prompt.suggestions.get(selected))?;
    Some((start, end, fragment, suggestion))
}

pub(super) fn current_expression_fragment(
    buffer: &str,
    cursor: usize,
) -> Option<(usize, usize, String)> {
    if cursor > buffer.len() {
        return None;
    }
    let chars: Vec<(usize, char)> = buffer.char_indices().collect();
    let char_cursor = chars
        .iter()
        .take_while(|(offset, _)| *offset < cursor)
        .count();
    let initial_depth = chars[..char_cursor]
        .iter()
        .fold(0usize, |depth, (_, ch)| match ch {
            '[' => depth + 1,
            ']' => depth.saturating_sub(1),
            _ => depth,
        });

    let mut start = cursor;
    let mut depth = initial_depth;
    let mut idx = char_cursor;
    while idx > 0 {
        let (offset, ch) = chars[idx - 1];
        let is_delimiter =
            depth == 0 && (ch.is_whitespace() || matches!(ch, '+' | '-' | '*' | ',' | '(' | ')'));
        if is_delimiter {
            break;
        }
        start = offset;
        match ch {
            ']' => depth += 1,
            '[' => depth = depth.saturating_sub(1),
            _ => {}
        }
        idx -= 1;
    }

    let mut end = cursor;
    let mut depth = initial_depth;
    let mut idx = char_cursor;
    while idx < chars.len() {
        let (_, ch) = chars[idx];
        let is_delimiter =
            depth == 0 && (ch.is_whitespace() || matches!(ch, '+' | '-' | '*' | ',' | '(' | ')'));
        if is_delimiter {
            break;
        }
        end = chars
            .get(idx + 1)
            .map(|(next_offset, _)| *next_offset)
            .unwrap_or(buffer.len());
        match ch {
            '[' => depth += 1,
            ']' => depth = depth.saturating_sub(1),
            _ => {}
        }
        idx += 1;
    }

    if start >= end || !matches!(buffer[start..].chars().next(), Some('$' | '!' | '#')) {
        return None;
    }
    Some((start, end, buffer[start..end].to_string()))
}

pub(super) fn expression_prompt_suggestions(
    state: &MultiChartState,
    file: Option<&File>,
    buffer: &str,
    cursor: usize,
) -> Vec<ExpressionPromptSuggestion> {
    let Some((_, _, fragment)) = current_expression_fragment(buffer, cursor) else {
        return Vec::new();
    };
    if fragment.starts_with('$') {
        let mut suggestions = state
            .items
            .iter()
            .filter_map(|item| {
                let label = format!("${}", item.id.0);
                let score = expression_suggestion_score(&label, &fragment, None)?;
                Some((score, item, label))
            })
            .map(|(score, item, label)| {
                (
                    score,
                    ExpressionPromptSuggestion {
                        symbol: match &item.source {
                            ChartSource::DatasetSelection(source) => match source.kind {
                                DatasetChartKind::Dataset => {
                                    configure::configured_symbol(|symbols| {
                                        symbols.tree.dataset_icon
                                    })
                                    .to_string()
                                }
                                DatasetChartKind::CompoundLeaf => {
                                    configure::configured_symbol(|symbols| {
                                        symbols.tree.compound_leaf_icon
                                    })
                                    .to_string()
                                }
                            },
                            ChartSource::DerivedExpression { .. } => {
                                configure::configured_symbol(|symbols| {
                                    symbols.chart.membership_marker
                                })
                                .to_string()
                            }
                        },
                        insert_text: label.clone(),
                        label: label.clone(),
                        detail: format!("{} | len {}", item.list_label(), item.series.len()),
                        kind: match &item.source {
                            ChartSource::DatasetSelection(source) => match source.kind {
                                DatasetChartKind::Dataset => {
                                    ExpressionPromptSuggestionKind::Dataset
                                }
                                DatasetChartKind::CompoundLeaf => {
                                    ExpressionPromptSuggestionKind::CompoundLeaf
                                }
                            },
                            ChartSource::DerivedExpression { .. } => {
                                ExpressionPromptSuggestionKind::ItemRef
                            }
                        },
                        highlight_spans: fuzzy_highlight_spans(&label, &fragment),
                    },
                )
            })
            .collect::<Vec<_>>();
        suggestions.sort_by(|(lhs_score, lhs), (rhs_score, rhs)| {
            rhs_score
                .cmp(lhs_score)
                .then_with(|| lhs.label.cmp(&rhs.label))
        });
        return suggestions
            .into_iter()
            .take(EXPRESSION_PROMPT_VISIBLE_SUGGESTIONS)
            .map(|(_, suggestion)| suggestion)
            .collect();
    }

    if !(fragment.starts_with('!') || fragment.starts_with('#')) {
        return Vec::new();
    }

    let Some(file) = file else {
        return Vec::new();
    };

    if let Some((target, attr_prefix)) = fragment.split_once(':') {
        let object_path = resolve_completion_target_path(state, target);
        let Some(object_path) = object_path else {
            return Vec::new();
        };
        return expression_attribute_suggestions(file, &object_path, target, attr_prefix);
    }

    expression_path_suggestions(file, &fragment)
}

pub(super) fn expression_suggestion_score(
    candidate: &str,
    query: &str,
    basename: Option<&str>,
) -> Option<i64> {
    if query.is_empty() {
        return Some(0);
    }
    let mut score = fuzzy_match_score(candidate, query)?;
    if candidate.starts_with(query) {
        score += 10_000;
    }
    if let Some(basename) = basename {
        let trimmed_query = query.trim_start_matches(&['!', '#', '/'][..]);
        if !trimmed_query.is_empty() && basename.starts_with(trimmed_query) {
            score += 5_000;
        }
    }
    Some(score)
}

fn shift_highlight_spans(spans: Vec<Range<usize>>, offset: usize) -> Vec<Range<usize>> {
    spans
        .into_iter()
        .map(|span| (span.start + offset)..(span.end + offset))
        .collect()
}

fn resolve_completion_target_path(state: &MultiChartState, target: &str) -> Option<String> {
    if let Some(path) = target
        .strip_prefix("!$")
        .or_else(|| target.strip_prefix("#$"))
    {
        let id = path.parse::<u64>().ok()?;
        return state
            .item_by_id(ChartItemId(id))
            .and_then(|item| item.source.dataset_source())
            .map(|source| source.dataset_path.clone());
    }
    if let Some(path) = target
        .strip_prefix('!')
        .or_else(|| target.strip_prefix('#'))
    {
        return normalize_absolute_object_path(path).ok();
    }
    None
}

fn expression_attribute_suggestions(
    file: &File,
    object_path: &str,
    target: &str,
    attr_prefix: &str,
) -> Vec<ExpressionPromptSuggestion> {
    let names = if object_path == "/" {
        file.attr_names().ok()
    } else if let Ok(group) = file.group(object_path) {
        group.attr_names().ok()
    } else if let Ok(dataset) = file.dataset(object_path) {
        dataset.attr_names().ok()
    } else {
        None
    }
    .unwrap_or_default();

    let mut suggestions = names
        .into_iter()
        .filter_map(|name| {
            let score = expression_suggestion_score(&name, attr_prefix, Some(&name))?;
            let label = format!("{target}:{name}");
            let highlight_spans =
                shift_highlight_spans(fuzzy_highlight_spans(&name, attr_prefix), target.len() + 1);
            Some((
                score,
                ExpressionPromptSuggestion {
                    symbol: String::new(),
                    insert_text: label.clone(),
                    label,
                    detail: String::new(),
                    kind: ExpressionPromptSuggestionKind::Attribute,
                    highlight_spans,
                },
            ))
        })
        .collect::<Vec<_>>();
    suggestions.sort_by(|(lhs_score, lhs), (rhs_score, rhs)| {
        rhs_score
            .cmp(lhs_score)
            .then_with(|| lhs.label.cmp(&rhs.label))
    });
    suggestions
        .into_iter()
        .take(EXPRESSION_PROMPT_VISIBLE_SUGGESTIONS)
        .map(|(_, suggestion)| suggestion)
        .collect()
}

fn expression_path_suggestions(file: &File, fragment: &str) -> Vec<ExpressionPromptSuggestion> {
    let target_kind = match fragment.chars().next() {
        Some('!') => Some(ExpressionAbsolutePathKind::Dataset),
        Some('#') => Some(ExpressionAbsolutePathKind::Dataset),
        _ => None,
    };
    let mut suggestions = expression_absolute_path_entries(file)
        .into_iter()
        .filter_map(|entry| {
            let kind_matches = match target_kind {
                Some(ExpressionAbsolutePathKind::Dataset) => true,
                Some(ExpressionAbsolutePathKind::Group) => {
                    entry.kind == ExpressionAbsolutePathKind::Group
                }
                None => true,
            };
            if !kind_matches {
                return None;
            }
            let label = format!("{}{}", &fragment[..1], entry.path);
            let basename = entry.path.rsplit('/').next();
            let score = expression_suggestion_score(&label, fragment, basename)?;
            let insert_text = match (&fragment[..1], entry.kind, entry.shape.as_ref()) {
                ("!", ExpressionAbsolutePathKind::Dataset, Some(shape)) if !shape.is_empty() => {
                    format!(
                        "{}{}[{}]",
                        &fragment[..1],
                        entry.path,
                        vec![".."; shape.len()].join(",")
                    )
                }
                _ => label.clone(),
            };
            Some((
                score,
                ExpressionPromptSuggestion {
                    symbol: match entry.kind {
                        ExpressionAbsolutePathKind::Group => {
                            configure::configured_symbol(|symbols| symbols.tree.folder_closed_leaf)
                                .to_string()
                        }
                        ExpressionAbsolutePathKind::Dataset => {
                            configure::configured_symbol(|symbols| symbols.tree.dataset_icon)
                                .to_string()
                        }
                    },
                    insert_text,
                    label: label.clone(),
                    detail: entry.detail,
                    kind: match entry.kind {
                        ExpressionAbsolutePathKind::Group => ExpressionPromptSuggestionKind::Group,
                        ExpressionAbsolutePathKind::Dataset => {
                            ExpressionPromptSuggestionKind::Dataset
                        }
                    },
                    highlight_spans: fuzzy_highlight_spans(&label, fragment),
                },
            ))
        })
        .collect::<Vec<_>>();
    suggestions.sort_by(|(lhs_score, lhs), (rhs_score, rhs)| {
        rhs_score
            .cmp(lhs_score)
            .then_with(|| lhs.label.cmp(&rhs.label))
    });
    suggestions
        .into_iter()
        .take(EXPRESSION_PROMPT_VISIBLE_SUGGESTIONS)
        .map(|(_, suggestion)| suggestion)
        .collect()
}

fn expression_absolute_path_entries(file: &File) -> Vec<ExpressionAbsolutePathEntry> {
    let Ok(root) = file.as_group() else {
        return Vec::new();
    };
    full_traversal(&root)
        .into_iter()
        .filter_map(|path| {
            if let Ok(dataset) = file.dataset(&path) {
                let shape = dataset.shape();
                Some(ExpressionAbsolutePathEntry {
                    detail: format_shape_suffix(&shape),
                    path,
                    kind: ExpressionAbsolutePathKind::Dataset,
                    shape: Some(shape),
                })
            } else if file.group(&path).is_ok() {
                Some(ExpressionAbsolutePathEntry {
                    path,
                    kind: ExpressionAbsolutePathKind::Group,
                    shape: None,
                    detail: String::new(),
                })
            } else {
                None
            }
        })
        .collect()
}

fn format_shape_suffix(shape: &[usize]) -> String {
    format!(
        "[{}]",
        shape
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(",")
    )
}
