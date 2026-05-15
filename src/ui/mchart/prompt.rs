use std::{collections::HashMap, ops::Range};

use hdf5_metno::File;

use crate::{
    configure,
    search::{full_traversal, fuzzy_highlight_spans, fuzzy_match_score},
};

use super::eval::normalize_absolute_object_path;
use super::expression::{parse_expression_item_ref, parse_expression_load_ref};
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

#[derive(Debug, Clone, Default, PartialEq)]
struct ExpressionPromptLookupCache {
    absolute_path_entries: Option<Vec<ExpressionAbsolutePathEntry>>,
    attribute_names: HashMap<String, Vec<String>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ExpressionReferenceFunction {
    Load,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ExpressionCompletionContext {
    ItemRef {
        fragment: String,
    },
    CallTarget {
        function: ExpressionReferenceFunction,
        fragment: String,
        target_prefix: String,
    },
    CallAttribute {
        function: ExpressionReferenceFunction,
        fragment: String,
        target_prefix: String,
        attr_prefix: String,
    },
}

#[derive(Debug, Clone)]
struct ExpressionPromptAnalysis {
    messages: Vec<ExpressionPromptMessage>,
    suggestions: Vec<ExpressionPromptSuggestion>,
    input_segments: Vec<ExpressionPromptInputSegment>,
}

#[derive(Debug, Clone, PartialEq)]
pub(super) struct ExpressionPromptState {
    pub(super) item_id: ChartItemId,
    pub(super) name_buffer: String,
    pub(super) name_cursor: usize,
    pub(super) buffer: String,
    pub(super) cursor: usize,
    pub(super) focus: ExpressionPromptFocus,
    pub(super) mode: ExpressionPromptMode,
    pub(super) messages: Vec<ExpressionPromptMessage>,
    pub(super) suggestions: Vec<ExpressionPromptSuggestion>,
    pub(super) selected_suggestion: Option<usize>,
    pub(super) input_segments: Vec<ExpressionPromptInputSegment>,
    lookup_cache: ExpressionPromptLookupCache,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ExpressionPromptFocus {
    Name,
    Expression,
}

impl ExpressionPromptState {
    pub(super) fn new(
        item_id: ChartItemId,
        name_buffer: String,
        buffer: String,
        cursor: usize,
        mode: ExpressionPromptMode,
    ) -> Self {
        let name_cursor = name_buffer.len();
        Self {
            item_id,
            name_buffer,
            name_cursor,
            buffer,
            cursor,
            focus: ExpressionPromptFocus::Expression,
            mode,
            messages: Vec::new(),
            suggestions: Vec::new(),
            selected_suggestion: None,
            input_segments: Vec::new(),
            lookup_cache: ExpressionPromptLookupCache::default(),
        }
    }
}

impl MultiChartState {
    fn prompt_word_boundary_left(buffer: &str, cursor: usize) -> usize {
        let bytes = buffer.as_bytes();
        let mut cursor = cursor.min(bytes.len());
        while cursor > 0 && bytes[cursor - 1].is_ascii_whitespace() {
            cursor -= 1;
        }
        while cursor > 0 && !bytes[cursor - 1].is_ascii_whitespace() {
            cursor -= 1;
        }
        cursor
    }

    fn prompt_word_boundary_right(buffer: &str, cursor: usize) -> usize {
        let bytes = buffer.as_bytes();
        let mut cursor = cursor.min(bytes.len());
        while cursor < bytes.len() && bytes[cursor].is_ascii_whitespace() {
            cursor += 1;
        }
        while cursor < bytes.len() && !bytes[cursor].is_ascii_whitespace() {
            cursor += 1;
        }
        cursor
    }

    pub fn open_expression_prompt(&mut self) {
        let buffer = String::new();
        let cursor = buffer.len();
        self.expression_prompt = Some(ExpressionPromptState::new(
            ChartItemId(self.next_id),
            String::new(),
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
            selected.id,
            selected.name.clone().unwrap_or_default(),
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
            match prompt.focus {
                ExpressionPromptFocus::Name => {
                    prompt.name_buffer.insert(prompt.name_cursor, ch);
                    prompt.name_cursor += 1;
                }
                ExpressionPromptFocus::Expression => {
                    prompt.buffer.insert(prompt.cursor, ch);
                    prompt.cursor += 1;
                    prompt.selected_suggestion = None;
                }
            }
        }
    }

    pub fn expression_backspace(&mut self) {
        if let Some(prompt) = self.expression_prompt.as_mut() {
            match prompt.focus {
                ExpressionPromptFocus::Name => {
                    if prompt.name_cursor > 0 {
                        prompt.name_cursor -= 1;
                        prompt.name_buffer.remove(prompt.name_cursor);
                    }
                }
                ExpressionPromptFocus::Expression => {
                    if prompt.cursor > 0 {
                        prompt.cursor -= 1;
                        prompt.buffer.remove(prompt.cursor);
                        prompt.selected_suggestion = None;
                    }
                }
            }
        }
    }

    pub fn expression_delete(&mut self) {
        if let Some(prompt) = self.expression_prompt.as_mut() {
            match prompt.focus {
                ExpressionPromptFocus::Name => {
                    if prompt.name_cursor < prompt.name_buffer.len() {
                        prompt.name_buffer.remove(prompt.name_cursor);
                    }
                }
                ExpressionPromptFocus::Expression => {
                    if prompt.cursor < prompt.buffer.len() {
                        prompt.buffer.remove(prompt.cursor);
                        prompt.selected_suggestion = None;
                    }
                }
            }
        }
    }

    pub fn expression_move_left(&mut self) {
        if let Some(prompt) = self.expression_prompt.as_mut() {
            match prompt.focus {
                ExpressionPromptFocus::Name => {
                    prompt.name_cursor = prompt.name_cursor.saturating_sub(1);
                }
                ExpressionPromptFocus::Expression => {
                    prompt.cursor = prompt.cursor.saturating_sub(1);
                    prompt.selected_suggestion = None;
                }
            }
        }
    }

    pub fn expression_move_right(&mut self) {
        if let Some(prompt) = self.expression_prompt.as_mut() {
            match prompt.focus {
                ExpressionPromptFocus::Name => {
                    if prompt.name_cursor < prompt.name_buffer.len() {
                        prompt.name_cursor += 1;
                    }
                }
                ExpressionPromptFocus::Expression => {
                    if prompt.cursor < prompt.buffer.len() {
                        prompt.cursor += 1;
                    }
                    prompt.selected_suggestion = None;
                }
            }
        }
    }

    pub fn expression_move_word_left(&mut self) {
        if let Some(prompt) = self.expression_prompt.as_mut() {
            match prompt.focus {
                ExpressionPromptFocus::Name => {
                    prompt.name_cursor =
                        Self::prompt_word_boundary_left(&prompt.name_buffer, prompt.name_cursor);
                }
                ExpressionPromptFocus::Expression => {
                    prompt.cursor = Self::prompt_word_boundary_left(&prompt.buffer, prompt.cursor);
                    prompt.selected_suggestion = None;
                }
            }
        }
    }

    pub fn expression_move_word_right(&mut self) {
        if let Some(prompt) = self.expression_prompt.as_mut() {
            match prompt.focus {
                ExpressionPromptFocus::Name => {
                    prompt.name_cursor =
                        Self::prompt_word_boundary_right(&prompt.name_buffer, prompt.name_cursor);
                }
                ExpressionPromptFocus::Expression => {
                    prompt.cursor = Self::prompt_word_boundary_right(&prompt.buffer, prompt.cursor);
                    prompt.selected_suggestion = None;
                }
            }
        }
    }

    pub fn expression_move_to_start(&mut self) {
        if let Some(prompt) = self.expression_prompt.as_mut() {
            match prompt.focus {
                ExpressionPromptFocus::Name => prompt.name_cursor = 0,
                ExpressionPromptFocus::Expression => {
                    prompt.cursor = 0;
                    prompt.selected_suggestion = None;
                }
            }
        }
    }

    pub fn expression_move_to_end(&mut self) {
        if let Some(prompt) = self.expression_prompt.as_mut() {
            match prompt.focus {
                ExpressionPromptFocus::Name => prompt.name_cursor = prompt.name_buffer.len(),
                ExpressionPromptFocus::Expression => {
                    prompt.cursor = prompt.buffer.len();
                    prompt.selected_suggestion = None;
                }
            }
        }
    }

    pub fn expression_clear(&mut self) {
        if let Some(prompt) = self.expression_prompt.as_mut() {
            match prompt.focus {
                ExpressionPromptFocus::Name => {
                    prompt.name_buffer.clear();
                    prompt.name_cursor = 0;
                }
                ExpressionPromptFocus::Expression => {
                    prompt.buffer.clear();
                    prompt.cursor = 0;
                    prompt.selected_suggestion = None;
                }
            }
        }
    }

    pub fn expression_toggle_focus(&mut self) {
        if let Some(prompt) = self.expression_prompt.as_mut() {
            prompt.focus = match prompt.focus {
                ExpressionPromptFocus::Name => ExpressionPromptFocus::Expression,
                ExpressionPromptFocus::Expression => ExpressionPromptFocus::Name,
            };
            prompt.selected_suggestion = None;
        }
    }

    pub(super) fn expression_set_focus_cursor(
        &mut self,
        focus: ExpressionPromptFocus,
        cursor: usize,
    ) {
        if let Some(prompt) = self.expression_prompt.as_mut() {
            prompt.focus = focus;
            match focus {
                ExpressionPromptFocus::Name => {
                    prompt.name_cursor = cursor.min(prompt.name_buffer.len());
                }
                ExpressionPromptFocus::Expression => {
                    prompt.cursor = cursor.min(prompt.buffer.len());
                    prompt.selected_suggestion = None;
                }
            }
        }
    }

    pub fn expression_select_next_suggestion(&mut self) {
        if let Some(prompt) = self.expression_prompt.as_mut() {
            if prompt.focus != ExpressionPromptFocus::Expression {
                return;
            }
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
            if prompt.focus != ExpressionPromptFocus::Expression {
                return;
            }
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
        if prompt.focus != ExpressionPromptFocus::Expression {
            return false;
        }
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
        let Some((buffer, cursor, selected_suggestion, lookup_cache)) =
            self.expression_prompt.as_ref().map(|prompt| {
                (
                    prompt.buffer.clone(),
                    prompt.cursor,
                    prompt.selected_suggestion,
                    prompt.lookup_cache.clone(),
                )
            })
        else {
            return;
        };
        let mut lookup_cache = lookup_cache;
        let analysis = expression_prompt_analysis(self, file, &buffer, cursor, &mut lookup_cache);
        if let Some(prompt) = self.expression_prompt.as_mut() {
            prompt.messages = analysis.messages;
            prompt.suggestions = analysis.suggestions;
            prompt.selected_suggestion =
                selected_suggestion.filter(|selected| *selected < prompt.suggestions.len());
            prompt.input_segments = analysis.input_segments;
            prompt.lookup_cache = lookup_cache;
        }
    }

    pub fn submit_expression_prompt(&mut self, file: Option<&File>) -> Result<(), String> {
        let (expression, name, mode, item_id) = self
            .expression_prompt
            .as_ref()
            .map(|prompt| {
                (
                    prompt.buffer.trim().to_string(),
                    prompt.name_buffer.clone(),
                    prompt.mode.clone(),
                    prompt.item_id,
                )
            })
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
                self.set_selected_item_name(&name, Some(item_id))?;
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

#[allow(dead_code)]
pub(super) fn expression_prompt_messages(
    state: &MultiChartState,
    file: Option<&File>,
    buffer: &str,
) -> Vec<ExpressionPromptMessage> {
    let mut cache = ExpressionPromptLookupCache::default();
    expression_prompt_analysis(state, file, buffer, buffer.len(), &mut cache).messages
}

fn expression_prompt_analysis(
    state: &MultiChartState,
    file: Option<&File>,
    buffer: &str,
    cursor: usize,
    cache: &mut ExpressionPromptLookupCache,
) -> ExpressionPromptAnalysis {
    let suggestions = expression_prompt_suggestions_with_cache(state, file, buffer, cursor, cache);
    let messages = expression_prompt_messages_with_cache(state, file, buffer, cursor, &suggestions);
    let input_segments = expression_prompt_input_segments(state, file, buffer);
    ExpressionPromptAnalysis {
        messages,
        suggestions,
        input_segments,
    }
}

fn expression_prompt_messages_with_cache(
    state: &MultiChartState,
    file: Option<&File>,
    buffer: &str,
    cursor: usize,
    suggestions: &[ExpressionPromptSuggestion],
) -> Vec<ExpressionPromptMessage> {
    let trimmed = buffer.trim();
    if trimmed.is_empty() {
        return vec![
            ExpressionPromptMessage {
                kind: ExpressionPromptMessageKind::Hint,
                text: "Use $1, load(/path)[..,0], load(/path:ATTR), or ($1, load(/time)[..])"
                    .to_string(),
            },
            ExpressionPromptMessage {
                kind: ExpressionPromptMessageKind::Hint,
                text:
                    "Tab switches between name and expression; Enter applies a selected suggestion or submits."
                        .to_string(),
            },
        ];
    }

    if expression_prompt_has_pending_completion(buffer, cursor, suggestions) {
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

    match state.validate_expression_with_file(trimmed, file) {
        Ok(validated) => {
            let result_kind = match validated.kind {
                DerivedExpressionKind::YSeries => "y-series",
                DerivedExpressionKind::XySeries => "x/y series",
                DerivedExpressionKind::Scalar => "scalar",
            };
            vec![ExpressionPromptMessage {
                kind: ExpressionPromptMessageKind::Valid,
                text: format!(
                    "Valid {result_kind} with {} samples",
                    validated.sample_count
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
    buffer: &str,
    cursor: usize,
    suggestions: &[ExpressionPromptSuggestion],
) -> bool {
    matches!(
        current_expression_fragment(buffer, cursor),
        Some((_, end, fragment)) if end == buffer.len() && !fragment.is_empty() && !suggestions.is_empty()
    )
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
        if ch != '$' && is_expression_function_start(&chars, idx).is_none() {
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

fn expression_function_name(function: ExpressionReferenceFunction) -> &'static str {
    match function {
        ExpressionReferenceFunction::Load => "load",
    }
}

fn is_expression_function_start(
    chars: &[(usize, char)],
    start_idx: usize,
) -> Option<ExpressionReferenceFunction> {
    let function = ExpressionReferenceFunction::Load;
    if start_idx > 0 {
        let prev = chars[start_idx - 1].1;
        if prev.is_ascii_alphanumeric() || prev == '_' {
            return None;
        }
    }
    let ident = chars[start_idx..]
        .iter()
        .take(4)
        .map(|(_, ch)| *ch)
        .collect::<String>();
    if ident != "load" {
        return None;
    }
    let mut idx = start_idx + 4;
    while idx < chars.len() && chars[idx].1.is_whitespace() {
        idx += 1;
    }
    (chars.get(idx).map(|(_, ch)| *ch) == Some('(')).then_some(function)
}

fn expression_function_open_paren_index(
    chars: &[(usize, char)],
    start_idx: usize,
) -> Option<usize> {
    is_expression_function_start(chars, start_idx)?;
    let mut idx = start_idx + 4;
    while idx < chars.len() && chars[idx].1.is_whitespace() {
        idx += 1;
    }
    Some(idx)
}

pub(super) fn consume_expression_reference_fragment(
    buffer: &str,
    chars: &[(usize, char)],
    start_idx: usize,
) -> usize {
    let start_char = chars[start_idx].1;
    if start_char == '$' {
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
                || ch == '/';
            if is_delimiter {
                break;
            }
            cursor += 1;
        }
        return chars
            .get(cursor)
            .map(|(offset, _)| *offset)
            .unwrap_or(buffer.len());
    }

    let Some(open_paren_idx) = expression_function_open_paren_index(chars, start_idx) else {
        return chars[start_idx].0;
    };
    let mut cursor = open_paren_idx + 1;
    let mut bracket_depth = 0usize;
    let mut paren_depth = 1usize;
    while cursor < chars.len() {
        let ch = chars[cursor].1;
        match ch {
            '[' => bracket_depth += 1,
            ']' => bracket_depth = bracket_depth.saturating_sub(1),
            '(' if bracket_depth == 0 => paren_depth += 1,
            ')' if bracket_depth == 0 => {
                paren_depth = paren_depth.saturating_sub(1);
                cursor += 1;
                if paren_depth == 0 {
                    break;
                }
                continue;
            }
            _ => {}
        }
        cursor += 1;
    }
    while cursor < chars.len() && chars[cursor].1 == '[' {
        cursor += 1;
        let mut selector_depth = 1usize;
        while cursor < chars.len() {
            match chars[cursor].1 {
                '[' => selector_depth += 1,
                ']' => {
                    selector_depth = selector_depth.saturating_sub(1);
                    cursor += 1;
                    if selector_depth == 0 {
                        break;
                    }
                    continue;
                }
                _ => {}
            }
            cursor += 1;
        }
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
            let _ = super::resolve_expression_item_value(
                state,
                &item_ref,
                super::ExpressionSeriesResolution::Overview,
            )?;
            Ok(())
        }
        Some('l') if fragment.starts_with("load") => {
            let mut chars = fragment[4..].chars().peekable();
            let load_ref = parse_expression_load_ref(&mut chars)?;
            if chars.next().is_some() {
                return Err(format!("Invalid load reference {fragment}"));
            }
            let file = file.ok_or_else(|| "No file loaded for load(...) references".to_string())?;
            super::validate_expression_load_ref(
                state,
                file,
                &load_ref,
                super::ExpressionSeriesResolution::Overview,
                true,
            )
            .map(|_| ())
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
    let mut idx = 0;
    while idx < chars.len() {
        let start = chars[idx].0;
        if chars[idx].1 != '$' && is_expression_function_start(&chars, idx).is_none() {
            idx += 1;
            continue;
        }
        let end = consume_expression_reference_fragment(buffer, &chars, idx);
        if start < end && cursor >= start && cursor <= end {
            return Some((start, end, buffer[start..end].to_string()));
        }
        while idx < chars.len() && chars[idx].0 < end {
            idx += 1;
        }
    }
    None
}

#[allow(dead_code)]
pub(super) fn expression_prompt_suggestions(
    state: &MultiChartState,
    file: Option<&File>,
    buffer: &str,
    cursor: usize,
) -> Vec<ExpressionPromptSuggestion> {
    let mut cache = ExpressionPromptLookupCache::default();
    expression_prompt_suggestions_with_cache(state, file, buffer, cursor, &mut cache)
}

fn expression_prompt_suggestions_with_cache(
    state: &MultiChartState,
    file: Option<&File>,
    buffer: &str,
    cursor: usize,
    cache: &mut ExpressionPromptLookupCache,
) -> Vec<ExpressionPromptSuggestion> {
    match current_expression_completion_context(buffer, cursor) {
        Some(ExpressionCompletionContext::ItemRef { fragment }) => {
            expression_item_ref_suggestions(state, &fragment)
        }
        Some(ExpressionCompletionContext::CallTarget {
            function,
            fragment,
            target_prefix,
        }) => {
            if target_prefix.starts_with('$') {
                return expression_item_target_suggestions(
                    state,
                    function,
                    &fragment,
                    &target_prefix,
                );
            }
            let Some(file) = file else {
                return Vec::new();
            };
            expression_path_suggestions(file, cache, function, &fragment, &target_prefix)
        }
        Some(ExpressionCompletionContext::CallAttribute {
            function,
            fragment,
            target_prefix,
            attr_prefix,
        }) => {
            let Some(file) = file else {
                return Vec::new();
            };
            let Some(object_path) = resolve_completion_target_path(state, &target_prefix) else {
                return Vec::new();
            };
            expression_attribute_suggestions(
                file,
                cache,
                function,
                &fragment,
                &object_path,
                &target_prefix,
                &attr_prefix,
            )
        }
        None => Vec::new(),
    }
}

fn current_expression_completion_context(
    buffer: &str,
    cursor: usize,
) -> Option<ExpressionCompletionContext> {
    let (start, _, fragment) = current_expression_fragment(buffer, cursor)?;
    if fragment.starts_with('$') {
        return Some(ExpressionCompletionContext::ItemRef { fragment });
    }
    let function = match fragment.chars().next()? {
        'l' if fragment.starts_with("load") => ExpressionReferenceFunction::Load,
        _ => return None,
    };
    let cursor_in_fragment = cursor.saturating_sub(start).min(fragment.len());
    let typed_prefix = fragment[..cursor_in_fragment].to_string();
    let open_paren = typed_prefix.find('(')?;
    if typed_prefix[open_paren + 1..].contains(')') {
        return None;
    }
    let inner = typed_prefix[open_paren + 1..].trim_start();
    if let Some((target_prefix, attr_prefix)) = inner.split_once(':') {
        return Some(ExpressionCompletionContext::CallAttribute {
            function,
            fragment,
            target_prefix: target_prefix.trim().to_string(),
            attr_prefix: attr_prefix.trim().to_string(),
        });
    }
    Some(ExpressionCompletionContext::CallTarget {
        function,
        fragment,
        target_prefix: inner.trim().to_string(),
    })
}

fn expression_item_ref_suggestions(
    state: &MultiChartState,
    fragment: &str,
) -> Vec<ExpressionPromptSuggestion> {
    let mut suggestions = state
        .items
        .iter()
        .flat_map(|item| {
            let mut labels = vec![format!("${}", item.id.0)];
            if let Some(name) = &item.name {
                labels.push(format!("${name}"));
            }
            labels.into_iter().filter_map(|label| {
                let score = expression_suggestion_score(&label, fragment, None)?;
                Some((score, item.clone(), label))
            })
        })
        .map(|(score, item, label)| {
            (
                score,
                ExpressionPromptSuggestion {
                    symbol: match &item.source {
                        ChartSource::DatasetSelection(source) => match source.kind {
                            DatasetChartKind::Dataset => {
                                configure::configured_symbol(|symbols| symbols.tree.dataset_icon)
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
                            configure::configured_symbol(|symbols| symbols.chart.membership_marker)
                                .to_string()
                        }
                    },
                    insert_text: label.clone(),
                    label: label.clone(),
                    detail: format!("{} | len {}", item.list_label(), item.series.len()),
                    kind: match &item.source {
                        ChartSource::DatasetSelection(source) => match source.kind {
                            DatasetChartKind::Dataset => ExpressionPromptSuggestionKind::Dataset,
                            DatasetChartKind::CompoundLeaf => {
                                ExpressionPromptSuggestionKind::CompoundLeaf
                            }
                        },
                        ChartSource::DerivedExpression { .. } => {
                            ExpressionPromptSuggestionKind::ItemRef
                        }
                    },
                    highlight_spans: fuzzy_highlight_spans(&label, fragment),
                },
            )
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
        let trimmed_query = query.trim_start_matches(&['l', 'o', 'a', 'd', '(', '/', '$'][..]);
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
    if let Some(path) = target.strip_prefix('$') {
        let id = path.parse::<u64>().ok()?;
        return state
            .item_by_id(ChartItemId(id))
            .and_then(|item| item.source.dataset_source())
            .map(|source| source.dataset_path.clone());
    }
    normalize_absolute_object_path(target).ok()
}

fn expression_item_target_suggestions(
    state: &MultiChartState,
    function: ExpressionReferenceFunction,
    fragment: &str,
    target_prefix: &str,
) -> Vec<ExpressionPromptSuggestion> {
    let function_name = expression_function_name(function);
    let mut suggestions = state
        .items
        .iter()
        .filter_map(|item| {
            let target = format!("${}", item.id.0);
            let score = expression_suggestion_score(&target, target_prefix, None)?;
            let label = format!("{function_name}({target})");
            Some((
                score,
                ExpressionPromptSuggestion {
                    symbol: configure::configured_symbol(|symbols| symbols.chart.membership_marker)
                        .to_string(),
                    insert_text: label.clone(),
                    label,
                    detail: format!("{} | len {}", item.list_label(), item.series.len()),
                    kind: ExpressionPromptSuggestionKind::ItemRef,
                    highlight_spans: fuzzy_highlight_spans(
                        &format!("{function_name}({target})"),
                        fragment,
                    ),
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

fn expression_attribute_suggestions(
    file: &File,
    cache: &mut ExpressionPromptLookupCache,
    function: ExpressionReferenceFunction,
    fragment: &str,
    object_path: &str,
    target_prefix: &str,
    attr_prefix: &str,
) -> Vec<ExpressionPromptSuggestion> {
    let names = cached_expression_attribute_names(file, cache, object_path);
    let function_name = expression_function_name(function);

    let mut suggestions = names
        .into_iter()
        .filter_map(|name| {
            let score = expression_suggestion_score(&name, attr_prefix, Some(&name))?;
            let label = format!("{function_name}({target_prefix}:{name})");
            let highlight_spans = shift_highlight_spans(
                fuzzy_highlight_spans(&name, attr_prefix),
                fragment.find(':').unwrap_or(fragment.len()) + 1,
            );
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

fn expression_path_suggestions(
    file: &File,
    cache: &mut ExpressionPromptLookupCache,
    function: ExpressionReferenceFunction,
    fragment: &str,
    target_prefix: &str,
) -> Vec<ExpressionPromptSuggestion> {
    let function_name = expression_function_name(function);
    let mut suggestions = expression_absolute_path_entries(file, cache)
        .into_iter()
        .filter_map(|entry| {
            let label = match (function, entry.kind, entry.shape.as_ref()) {
                (
                    ExpressionReferenceFunction::Load,
                    ExpressionAbsolutePathKind::Dataset,
                    Some(shape),
                ) if !shape.is_empty() => {
                    format!(
                        "{function_name}({})[{}]",
                        entry.path,
                        vec![".."; shape.len()].join(",")
                    )
                }
                _ => format!("{function_name}({})", entry.path),
            };
            let basename = entry.path.rsplit('/').next();
            let score = expression_suggestion_score(&entry.path, target_prefix, basename)?;
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
                    insert_text: label.clone(),
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

fn expression_absolute_path_entries(
    file: &File,
    cache: &mut ExpressionPromptLookupCache,
) -> Vec<ExpressionAbsolutePathEntry> {
    if let Some(entries) = &cache.absolute_path_entries {
        return entries.clone();
    }
    let Ok(root) = file.as_group() else {
        return Vec::new();
    };
    let entries = full_traversal(&root)
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
        .collect::<Vec<_>>();
    cache.absolute_path_entries = Some(entries.clone());
    entries
}

fn cached_expression_attribute_names(
    file: &File,
    cache: &mut ExpressionPromptLookupCache,
    object_path: &str,
) -> Vec<String> {
    if let Some(names) = cache.attribute_names.get(object_path) {
        return names.clone();
    }
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
    cache
        .attribute_names
        .insert(object_path.to_string(), names.clone());
    names
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
