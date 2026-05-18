use std::ops::Range;

use super::*;
use hdf5_metno::File;

mod completion;

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
    Function,
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

use completion::{
    current_expression_completion, expression_prompt_analysis, ExpressionPromptLookupCache,
};

#[cfg(test)]
pub(super) fn consume_expression_reference_fragment(
    buffer: &str,
    chars: &[(usize, char)],
    start_idx: usize,
) -> usize {
    completion::consume_expression_reference_fragment(buffer, chars, start_idx)
}

#[cfg(test)]
pub(super) fn current_expression_fragment(
    buffer: &str,
    cursor: usize,
) -> Option<(usize, usize, String)> {
    completion::current_expression_fragment(buffer, cursor)
}

#[cfg(test)]
pub(super) fn expression_prompt_messages(
    state: &MultiChartState,
    file: Option<&File>,
    buffer: &str,
) -> Vec<ExpressionPromptMessage> {
    completion::expression_prompt_messages(state, file, buffer)
}

#[cfg(test)]
pub(super) fn expression_suggestion_score(
    candidate: &str,
    query: &str,
    basename: Option<&str>,
) -> Option<i64> {
    completion::expression_suggestion_score(candidate, query, basename)
}

impl ExpressionPromptState {
    pub(super) fn new(
        item_id: ChartItemId,
        name_buffer: String,
        buffer: String,
        cursor: usize,
        mode: ExpressionPromptMode,
    ) -> Self {
        let cursor = MultiChartState::snap_prompt_cursor(&buffer, cursor);
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
    pub(super) fn snap_prompt_cursor(buffer: &str, cursor: usize) -> usize {
        let mut cursor = cursor.min(buffer.len());
        while cursor > 0 && !buffer.is_char_boundary(cursor) {
            cursor -= 1;
        }
        cursor
    }

    pub(super) fn prompt_cursor_from_char_offset(buffer: &str, char_offset: usize) -> usize {
        buffer
            .char_indices()
            .nth(char_offset)
            .map(|(idx, _)| idx)
            .unwrap_or(buffer.len())
    }

    pub(super) fn prompt_cursor_char_offset(buffer: &str, cursor: usize) -> usize {
        let cursor = Self::snap_prompt_cursor(buffer, cursor);
        buffer[..cursor].chars().count()
    }

    fn prompt_previous_boundary(buffer: &str, cursor: usize) -> usize {
        let cursor = Self::snap_prompt_cursor(buffer, cursor);
        buffer[..cursor]
            .char_indices()
            .last()
            .map(|(idx, _)| idx)
            .unwrap_or(0)
    }

    fn prompt_next_boundary(buffer: &str, cursor: usize) -> usize {
        let cursor = Self::snap_prompt_cursor(buffer, cursor);
        buffer[cursor..]
            .chars()
            .next()
            .map(|ch| cursor + ch.len_utf8())
            .unwrap_or(buffer.len())
    }

    fn prompt_word_boundary_left(buffer: &str, cursor: usize) -> usize {
        let mut cursor = Self::snap_prompt_cursor(buffer, cursor);
        while cursor > 0 {
            let previous = Self::prompt_previous_boundary(buffer, cursor);
            let Some(ch) = buffer[previous..cursor].chars().next() else {
                break;
            };
            if !ch.is_whitespace() {
                break;
            }
            cursor = previous;
        }
        while cursor > 0 {
            let previous = Self::prompt_previous_boundary(buffer, cursor);
            let Some(ch) = buffer[previous..cursor].chars().next() else {
                break;
            };
            if ch.is_whitespace() {
                break;
            }
            cursor = previous;
        }
        cursor
    }

    fn prompt_word_boundary_right(buffer: &str, cursor: usize) -> usize {
        let mut cursor = Self::snap_prompt_cursor(buffer, cursor);
        while cursor < buffer.len() {
            let next = Self::prompt_next_boundary(buffer, cursor);
            let Some(ch) = buffer[cursor..next].chars().next() else {
                break;
            };
            if !ch.is_whitespace() {
                break;
            }
            cursor = next;
        }
        while cursor < buffer.len() {
            let next = Self::prompt_next_boundary(buffer, cursor);
            let Some(ch) = buffer[cursor..next].chars().next() else {
                break;
            };
            if ch.is_whitespace() {
                break;
            }
            cursor = next;
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
                    let cursor = Self::snap_prompt_cursor(&prompt.name_buffer, prompt.name_cursor);
                    prompt.name_buffer.insert(cursor, ch);
                    prompt.name_cursor = cursor + ch.len_utf8();
                }
                ExpressionPromptFocus::Expression => {
                    let cursor = Self::snap_prompt_cursor(&prompt.buffer, prompt.cursor);
                    prompt.buffer.insert(cursor, ch);
                    prompt.cursor = cursor + ch.len_utf8();
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
                        let cursor =
                            Self::prompt_previous_boundary(&prompt.name_buffer, prompt.name_cursor);
                        prompt.name_buffer.remove(cursor);
                        prompt.name_cursor = cursor;
                    }
                }
                ExpressionPromptFocus::Expression => {
                    if prompt.cursor > 0 {
                        let cursor = Self::prompt_previous_boundary(&prompt.buffer, prompt.cursor);
                        prompt.buffer.remove(cursor);
                        prompt.cursor = cursor;
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
                    let cursor = Self::snap_prompt_cursor(&prompt.name_buffer, prompt.name_cursor);
                    if cursor < prompt.name_buffer.len() {
                        prompt.name_buffer.remove(cursor);
                        prompt.name_cursor = cursor;
                    }
                }
                ExpressionPromptFocus::Expression => {
                    let cursor = Self::snap_prompt_cursor(&prompt.buffer, prompt.cursor);
                    if cursor < prompt.buffer.len() {
                        prompt.buffer.remove(cursor);
                        prompt.cursor = cursor;
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
                    prompt.name_cursor =
                        Self::prompt_previous_boundary(&prompt.name_buffer, prompt.name_cursor);
                }
                ExpressionPromptFocus::Expression => {
                    prompt.cursor = Self::prompt_previous_boundary(&prompt.buffer, prompt.cursor);
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
                        prompt.name_cursor =
                            Self::prompt_next_boundary(&prompt.name_buffer, prompt.name_cursor);
                    }
                }
                ExpressionPromptFocus::Expression => {
                    if prompt.cursor < prompt.buffer.len() {
                        prompt.cursor = Self::prompt_next_boundary(&prompt.buffer, prompt.cursor);
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
                    prompt.name_cursor = Self::snap_prompt_cursor(&prompt.name_buffer, cursor);
                }
                ExpressionPromptFocus::Expression => {
                    prompt.cursor = Self::snap_prompt_cursor(&prompt.buffer, cursor);
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
