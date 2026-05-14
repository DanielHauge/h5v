use crate::data::{PreviewSelection, SliceSelection};

use super::ChartItemId;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ExprBinaryOp {
    Add,
    Sub,
    Mul,
    Div,
}

#[derive(Debug, Clone, PartialEq)]
pub(super) enum ExpressionAst {
    Number(f64),
    ItemRef(ExpressionItemRef),
    SeriesRef(ExpressionSeriesRef),
    ScalarRef(ExpressionScalarRef),
    UnaryMinus(Box<ExpressionAst>),
    Binary {
        op: ExprBinaryOp,
        lhs: Box<ExpressionAst>,
        rhs: Box<ExpressionAst>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub(super) enum ExpressionToken {
    ItemRef(ExpressionItemRef),
    SeriesRef(ExpressionSeriesRef),
    ScalarRef(ExpressionScalarRef),
    Number(f64),
    Plus,
    Minus,
    Star,
    Slash,
    Comma,
    LParen,
    RParen,
}

#[derive(Debug, Clone, PartialEq)]
pub(super) enum ParsedExpression {
    YSeries(ExpressionAst),
    XySeries(ExpressionAst, ExpressionAst),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(super) enum ExpressionDatasetSelector {
    All,
    Index(usize),
    Slice {
        start: Option<usize>,
        end: Option<usize>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(super) enum ExpressionObjectTarget {
    AbsolutePath(String),
    ItemRef(ChartItemId),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(super) struct ExpressionItemSlice {
    pub(super) start: usize,
    pub(super) end: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(super) struct ExpressionItemRef {
    pub(super) id: ChartItemId,
    pub(super) slice: Option<ExpressionItemSlice>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(super) struct ExpressionSeriesRef {
    pub(super) target: ExpressionObjectTarget,
    pub(super) attr_name: Option<String>,
    pub(super) selectors: Option<Vec<ExpressionDatasetSelector>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(super) struct ExpressionScalarRef {
    pub(super) target: ExpressionObjectTarget,
    pub(super) attr_name: Option<String>,
}

impl ExpressionObjectTarget {
    pub(super) fn render(&self) -> String {
        match self {
            ExpressionObjectTarget::AbsolutePath(path) => path.clone(),
            ExpressionObjectTarget::ItemRef(id) => format!("${}", id.0),
        }
    }
}

impl ExpressionItemRef {
    pub(super) fn render(&self) -> String {
        match &self.slice {
            Some(slice) => format!("${}[{}..{}]", self.id.0, slice.start, slice.end),
            None => format!("${}", self.id.0),
        }
    }
}

impl ExpressionSeriesRef {
    pub(super) fn render(&self) -> String {
        let base = match &self.attr_name {
            Some(attr_name) => format!("!{}:{attr_name}", self.target.render()),
            None => format!("!{}", self.target.render()),
        };
        match &self.selectors {
            None => base,
            Some(selectors) => {
                let selectors = selectors
                    .iter()
                    .map(|selector| match selector {
                        ExpressionDatasetSelector::All => "..".to_string(),
                        ExpressionDatasetSelector::Index(index) => index.to_string(),
                        ExpressionDatasetSelector::Slice { start, end } => format!(
                            "{}..{}",
                            start.map(|value| value.to_string()).unwrap_or_default(),
                            end.map(|value| value.to_string()).unwrap_or_default()
                        ),
                    })
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("{base}[{selectors}]")
            }
        }
    }

    pub(super) fn to_preview_selection(&self, shape: &[usize]) -> Result<PreviewSelection, String> {
        let reference = self.render();
        if shape.is_empty() {
            return Err(format!(
                "Series reference {reference} must point to a non-scalar array"
            ));
        }
        let (x, index, slice) = match &self.selectors {
            None => {
                if shape.len() != 1 {
                    return Err(format!(
                        "Series reference {reference} needs an explicit selector like !/path[..,0] for rank-{} arrays",
                        shape.len()
                    ));
                }
                (0, vec![0], SliceSelection::All)
            }
            Some(selectors) => {
                if selectors.len() != shape.len() {
                    return Err(format!(
                        "Dataset reference {} must provide exactly {} selectors",
                        self.render(),
                        shape.len()
                    ));
                }
                let mut x = None;
                let mut index = vec![0; shape.len()];
                let mut slice = SliceSelection::All;
                for (dim, selector) in selectors.iter().enumerate() {
                    match selector {
                        ExpressionDatasetSelector::All => {
                            if x.replace(dim).is_some() {
                                return Err(format!(
                                    "Dataset reference {} must contain exactly one slice axis selector",
                                    self.render()
                                ));
                            }
                        }
                        ExpressionDatasetSelector::Index(selected) => {
                            if *selected >= shape[dim] {
                                return Err(format!(
                                    "Dataset reference {} selects index {} out of bounds for dim {} with length {}",
                                    self.render(),
                                    selected,
                                    dim,
                                    shape[dim]
                                ));
                            }
                            index[dim] = *selected;
                        }
                        ExpressionDatasetSelector::Slice { start, end } => {
                            if x.replace(dim).is_some() {
                                return Err(format!(
                                    "Dataset reference {} must contain exactly one slice axis selector",
                                    self.render()
                                ));
                            }
                            let start = start.unwrap_or(0);
                            let end = end.unwrap_or(shape[dim]);
                            if end <= start {
                                return Err(format!(
                                    "Dataset reference {} must use an increasing slice for dim {}",
                                    self.render(),
                                    dim
                                ));
                            }
                            if end > shape[dim] {
                                return Err(format!(
                                    "Dataset reference {} selects slice {}..{} out of bounds for dim {} with length {}",
                                    self.render(),
                                    start,
                                    end,
                                    dim,
                                    shape[dim]
                                ));
                            }
                            slice = SliceSelection::FromTo(start, end);
                        }
                    }
                }
                (
                    x.ok_or_else(|| {
                        format!(
                            "Series reference {reference} must contain exactly one slice axis selector"
                        )
                    })?,
                    index,
                    slice,
                )
            }
        };

        Ok(PreviewSelection { x, index, slice })
    }
}

impl ExpressionScalarRef {
    pub(super) fn render(&self) -> String {
        match &self.attr_name {
            Some(attr_name) => format!("#{}:{attr_name}", self.target.render()),
            None => format!("#{}", self.target.render()),
        }
    }
}

pub(super) fn tokenize_expression(input: &str) -> Result<Vec<ExpressionToken>, String> {
    let mut chars = input.chars().peekable();
    let mut tokens = Vec::new();

    while let Some(&ch) = chars.peek() {
        match ch {
            ' ' | '\t' => {
                chars.next();
            }
            '$' => {
                chars.next();
                tokens.push(ExpressionToken::ItemRef(parse_expression_item_ref(
                    &mut chars,
                )?));
            }
            '!' => {
                chars.next();
                tokens.push(ExpressionToken::SeriesRef(parse_expression_series_ref(
                    &mut chars,
                )?));
            }
            '#' => {
                chars.next();
                tokens.push(ExpressionToken::ScalarRef(parse_expression_scalar_ref(
                    &mut chars,
                )?));
            }
            '0'..='9' | '.' => {
                let mut number = String::new();
                let mut seen_dot = false;
                while let Some(next) = chars.peek() {
                    if next.is_ascii_digit() {
                        number.push(*next);
                        chars.next();
                    } else if *next == '.' && !seen_dot {
                        seen_dot = true;
                        number.push(*next);
                        chars.next();
                    } else {
                        break;
                    }
                }
                let value = number
                    .parse::<f64>()
                    .map_err(|_| format!("Invalid numeric literal '{number}'"))?;
                tokens.push(ExpressionToken::Number(value));
            }
            '+' => {
                chars.next();
                tokens.push(ExpressionToken::Plus);
            }
            '-' => {
                chars.next();
                tokens.push(ExpressionToken::Minus);
            }
            '*' => {
                chars.next();
                tokens.push(ExpressionToken::Star);
            }
            '/' => {
                chars.next();
                tokens.push(ExpressionToken::Slash);
            }
            ',' => {
                chars.next();
                tokens.push(ExpressionToken::Comma);
            }
            '(' => {
                chars.next();
                tokens.push(ExpressionToken::LParen);
            }
            ')' => {
                chars.next();
                tokens.push(ExpressionToken::RParen);
            }
            other => {
                return Err(format!(
                    "Unsupported character '{}' in expression. Use $id item references, !series references, #scalar references, numbers, + - * /, commas, and parentheses",
                    other
                ));
            }
        }
    }

    Ok(tokens)
}

fn parse_expression_item_id(
    chars: &mut std::iter::Peekable<std::str::Chars<'_>>,
) -> Result<ChartItemId, String> {
    let mut digits = String::new();
    while let Some(next) = chars.peek() {
        if next.is_ascii_digit() {
            digits.push(*next);
            chars.next();
        } else {
            break;
        }
    }
    if digits.is_empty() {
        return Err("Expected digits after '$' item reference".to_string());
    }
    let id = digits
        .parse::<u64>()
        .map_err(|_| format!("Invalid chart item reference '${digits}'"))?;
    Ok(ChartItemId(id))
}

pub(super) fn parse_expression_item_ref(
    chars: &mut std::iter::Peekable<std::str::Chars<'_>>,
) -> Result<ExpressionItemRef, String> {
    let id = parse_expression_item_id(chars)?;
    let slice = if chars.peek() == Some(&'[') {
        chars.next();
        let mut spec = String::new();
        let mut closed = false;
        for next in chars.by_ref() {
            if next == ']' {
                closed = true;
                break;
            }
            spec.push(next);
        }
        if !closed {
            return Err(format!(
                "Chart item reference '${}[{spec}' is missing a closing ']'",
                id.0
            ));
        }
        Some(parse_expression_item_slice(id, &spec)?)
    } else {
        None
    };
    Ok(ExpressionItemRef { id, slice })
}

fn parse_expression_item_slice(id: ChartItemId, spec: &str) -> Result<ExpressionItemSlice, String> {
    let Some((start, end)) = spec.split_once("..") else {
        return Err(format!(
            "Chart item reference '${}[{spec}]' must use a slice like [0..5]",
            id.0
        ));
    };
    let start = start.trim().parse::<usize>().map_err(|_| {
        format!(
            "Chart item reference '${}[{spec}]' has invalid slice start '{}'",
            id.0,
            start.trim()
        )
    })?;
    let end = end.trim().parse::<usize>().map_err(|_| {
        format!(
            "Chart item reference '${}[{spec}]' has invalid slice end '{}'",
            id.0,
            end.trim()
        )
    })?;
    if end <= start {
        return Err(format!(
            "Chart item reference '${}[{spec}]' must use an increasing slice",
            id.0
        ));
    }
    Ok(ExpressionItemSlice { start, end })
}

fn parse_expression_absolute_path(
    chars: &mut std::iter::Peekable<std::str::Chars<'_>>,
) -> Result<String, String> {
    let mut path = String::new();
    while let Some(&next) = chars.peek() {
        if next == '['
            || next == ':'
            || next.is_whitespace()
            || matches!(next, '+' | '-' | '*' | ',' | '(' | ')')
        {
            break;
        }
        path.push(next);
        chars.next();
    }
    if path.is_empty() {
        return Err("Expected an absolute HDF5 path beginning with '/'".to_string());
    }
    Ok(path)
}

fn parse_expression_object_target(
    chars: &mut std::iter::Peekable<std::str::Chars<'_>>,
    prefix: char,
) -> Result<ExpressionObjectTarget, String> {
    match chars.peek().copied() {
        Some('/') => Ok(ExpressionObjectTarget::AbsolutePath(parse_expression_absolute_path(
            chars,
        )?)),
        Some('$') => {
            chars.next();
            Ok(ExpressionObjectTarget::ItemRef(parse_expression_item_id(chars)?))
        }
        _ => Err(match prefix {
            '!' => {
                "Series references must use an absolute path like !/group/dataset or an item-backed attribute like !$1:ATTR"
                    .to_string()
            }
            '#' => {
                "Scalar references must use an absolute path like #/group/scalar or an item-backed attribute like #$1:ATTR"
                    .to_string()
            }
            _ => "Invalid expression reference".to_string(),
        }),
    }
}

pub(super) fn parse_expression_series_ref(
    chars: &mut std::iter::Peekable<std::str::Chars<'_>>,
) -> Result<ExpressionSeriesRef, String> {
    let target = parse_expression_object_target(chars, '!')?;
    let attr_name = if chars.peek() == Some(&':') {
        chars.next();
        let attr_name = parse_expression_attribute_name(chars);
        if attr_name.is_empty() {
            return Err(format!(
                "Expected an attribute name after '{}:' in series reference",
                target.render()
            ));
        }
        Some(attr_name)
    } else {
        None
    };

    if attr_name.is_some() && chars.peek() == Some(&'[') {
        return Err(
            "Series attribute references currently use the full attribute value and do not support selectors"
                .to_string(),
        );
    }

    let selectors = if chars.peek() == Some(&'[') {
        chars.next();
        let mut spec = String::new();
        let mut closed = false;
        for next in chars.by_ref() {
            if next == ']' {
                closed = true;
                break;
            }
            spec.push(next);
        }
        if !closed {
            return Err(format!(
                "Series reference '{}[{spec}' is missing a closing ']'",
                match &attr_name {
                    Some(attr_name) => format!("!{}:{attr_name}", target.render()),
                    None => format!("!{}", target.render()),
                }
            ));
        }
        Some(parse_expression_dataset_selectors(
            &match &attr_name {
                Some(attr_name) => format!("!{}:{attr_name}", target.render()),
                None => format!("!{}", target.render()),
            },
            &spec,
        )?)
    } else {
        None
    };

    Ok(ExpressionSeriesRef {
        target,
        attr_name,
        selectors,
    })
}

pub(super) fn parse_expression_scalar_ref(
    chars: &mut std::iter::Peekable<std::str::Chars<'_>>,
) -> Result<ExpressionScalarRef, String> {
    let target = parse_expression_object_target(chars, '#')?;
    let attr_name = if chars.peek() == Some(&':') {
        chars.next();
        let attr_name = parse_expression_attribute_name(chars);
        if attr_name.is_empty() {
            return Err(format!(
                "Expected an attribute name after '{}:' in scalar reference",
                target.render()
            ));
        }
        Some(attr_name)
    } else {
        None
    };

    if matches!(target, ExpressionObjectTarget::ItemRef(_)) && attr_name.is_none() {
        return Err("Scalar item references must name an attribute like #$1:OFFSET".to_string());
    }
    if chars.peek() == Some(&'[') {
        return Err("Scalar references cannot use series selectors".to_string());
    }

    Ok(ExpressionScalarRef { target, attr_name })
}

fn parse_expression_dataset_selectors(
    reference: &str,
    spec: &str,
) -> Result<Vec<ExpressionDatasetSelector>, String> {
    let parts = spec
        .split(',')
        .map(str::trim)
        .map(ToString::to_string)
        .collect::<Vec<_>>();
    if parts.is_empty() || parts.iter().any(|part| part.is_empty()) {
        return Err(format!(
            "Series reference '{reference}[{spec}]' must use comma-separated selectors like [..,0]"
        ));
    }

    parts
        .into_iter()
        .map(|part| {
            if part == ".." {
                Ok(ExpressionDatasetSelector::All)
            } else if let Some((start, end)) = part.split_once("..") {
                let start = if start.trim().is_empty() {
                    None
                } else {
                    Some(start.trim().parse::<usize>().map_err(|_| {
                        format!(
                            "Series reference '{reference}[{spec}]' has invalid slice start '{start}'; use '..', 'a..b', '..b', 'a..', or a non-negative integer"
                        )
                    })?)
                };
                let end = if end.trim().is_empty() {
                    None
                } else {
                    Some(end.trim().parse::<usize>().map_err(|_| {
                        format!(
                            "Series reference '{reference}[{spec}]' has invalid slice end '{end}'; use '..', 'a..b', '..b', 'a..', or a non-negative integer"
                        )
                    })?)
                };
                if start.is_none() && end.is_none() {
                    Ok(ExpressionDatasetSelector::All)
                } else {
                    Ok(ExpressionDatasetSelector::Slice { start, end })
                }
            } else {
                part.parse::<usize>()
                    .map(ExpressionDatasetSelector::Index)
                    .map_err(|_| {
                        format!(
                            "Series reference '{reference}[{spec}]' has invalid selector '{part}'; use '..', 'a..b', '..b', 'a..', or a non-negative integer"
                        )
                    })
            }
        })
        .collect()
}

fn parse_expression_attribute_name(chars: &mut std::iter::Peekable<std::str::Chars<'_>>) -> String {
    let mut attr_name = String::new();
    while let Some(&next) = chars.peek() {
        if next == '[' || next.is_whitespace() || matches!(next, '+' | '-' | '*' | '/' | '(' | ')')
        {
            break;
        }
        attr_name.push(next);
        chars.next();
    }
    attr_name
}

fn parse_expression(tokens: &[ExpressionToken]) -> Result<ExpressionAst, String> {
    fn parse_expr(tokens: &[ExpressionToken], pos: &mut usize) -> Result<ExpressionAst, String> {
        let mut expr = parse_term(tokens, pos)?;
        while *pos < tokens.len() {
            let op = match tokens[*pos] {
                ExpressionToken::Plus => ExprBinaryOp::Add,
                ExpressionToken::Minus => ExprBinaryOp::Sub,
                _ => break,
            };
            *pos += 1;
            let rhs = parse_term(tokens, pos)?;
            expr = ExpressionAst::Binary {
                op,
                lhs: Box::new(expr),
                rhs: Box::new(rhs),
            };
        }
        Ok(expr)
    }

    fn parse_term(tokens: &[ExpressionToken], pos: &mut usize) -> Result<ExpressionAst, String> {
        let mut expr = parse_factor(tokens, pos)?;
        while *pos < tokens.len() {
            let op = match tokens[*pos] {
                ExpressionToken::Star => ExprBinaryOp::Mul,
                ExpressionToken::Slash => ExprBinaryOp::Div,
                _ => break,
            };
            *pos += 1;
            let rhs = parse_factor(tokens, pos)?;
            expr = ExpressionAst::Binary {
                op,
                lhs: Box::new(expr),
                rhs: Box::new(rhs),
            };
        }
        Ok(expr)
    }

    fn parse_factor(tokens: &[ExpressionToken], pos: &mut usize) -> Result<ExpressionAst, String> {
        if *pos >= tokens.len() {
            return Err("Unexpected end of expression".to_string());
        }
        match &tokens[*pos] {
            ExpressionToken::Number(value) => {
                *pos += 1;
                Ok(ExpressionAst::Number(*value))
            }
            ExpressionToken::ItemRef(item_ref) => {
                *pos += 1;
                Ok(ExpressionAst::ItemRef(item_ref.clone()))
            }
            ExpressionToken::SeriesRef(series_ref) => {
                *pos += 1;
                Ok(ExpressionAst::SeriesRef(series_ref.clone()))
            }
            ExpressionToken::ScalarRef(scalar_ref) => {
                *pos += 1;
                Ok(ExpressionAst::ScalarRef(scalar_ref.clone()))
            }
            ExpressionToken::Minus => {
                *pos += 1;
                Ok(ExpressionAst::UnaryMinus(Box::new(parse_factor(
                    tokens, pos,
                )?)))
            }
            ExpressionToken::LParen => {
                *pos += 1;
                let expr = parse_expr(tokens, pos)?;
                if *pos >= tokens.len() || !matches!(tokens[*pos], ExpressionToken::RParen) {
                    return Err("Missing closing ')' in expression".to_string());
                }
                *pos += 1;
                Ok(expr)
            }
            other => Err(format!("Unexpected token '{other:?}' in expression")),
        }
    }

    let mut pos = 0;
    let expr = parse_expr(tokens, &mut pos)?;
    if pos != tokens.len() {
        return Err("Unexpected trailing tokens in expression".to_string());
    }
    Ok(expr)
}

pub(super) fn parse_derived_expression(
    tokens: &[ExpressionToken],
) -> Result<ParsedExpression, String> {
    if let Some((x_tokens, y_tokens)) = split_top_level_tuple(tokens) {
        let x_expr = parse_expression(x_tokens)?;
        let y_expr = parse_expression(y_tokens)?;
        return Ok(ParsedExpression::XySeries(x_expr, y_expr));
    }
    Ok(ParsedExpression::YSeries(parse_expression(tokens)?))
}

fn split_top_level_tuple(
    tokens: &[ExpressionToken],
) -> Option<(&[ExpressionToken], &[ExpressionToken])> {
    if tokens.len() < 5
        || !matches!(tokens.first(), Some(ExpressionToken::LParen))
        || !matches!(tokens.last(), Some(ExpressionToken::RParen))
    {
        return None;
    }

    let mut depth = 0usize;
    let mut comma_index = None;
    for (idx, token) in tokens.iter().enumerate() {
        match token {
            ExpressionToken::LParen => depth += 1,
            ExpressionToken::RParen => {
                depth = depth.saturating_sub(1);
                if depth == 0 && idx != tokens.len() - 1 {
                    return None;
                }
            }
            ExpressionToken::Comma if depth == 1 => {
                if comma_index.replace(idx).is_some() {
                    return None;
                }
            }
            _ => {}
        }
    }

    let comma_index = comma_index?;
    let x_tokens = &tokens[1..comma_index];
    let y_tokens = &tokens[comma_index + 1..tokens.len() - 1];
    if x_tokens.is_empty() || y_tokens.is_empty() {
        return None;
    }
    Some((x_tokens, y_tokens))
}

#[derive(Debug, Default)]
pub(super) struct ExpressionRefs {
    pub(super) item_refs: Vec<ExpressionItemRef>,
    pub(super) series_refs: Vec<ExpressionSeriesRef>,
    pub(super) scalar_refs: Vec<ExpressionScalarRef>,
}

fn collect_expression_refs(expr: &ExpressionAst, out: &mut ExpressionRefs) {
    match expr {
        ExpressionAst::Number(_) => {}
        ExpressionAst::ItemRef(item_ref) => out.item_refs.push(item_ref.clone()),
        ExpressionAst::SeriesRef(series_ref) => out.series_refs.push(series_ref.clone()),
        ExpressionAst::ScalarRef(scalar_ref) => out.scalar_refs.push(scalar_ref.clone()),
        ExpressionAst::UnaryMinus(inner) => collect_expression_refs(inner, out),
        ExpressionAst::Binary { lhs, rhs, .. } => {
            collect_expression_refs(lhs, out);
            collect_expression_refs(rhs, out);
        }
    }
}

pub(super) fn collect_parsed_expression_refs(expr: &ParsedExpression, out: &mut ExpressionRefs) {
    match expr {
        ParsedExpression::YSeries(ast) => collect_expression_refs(ast, out),
        ParsedExpression::XySeries(x_ast, y_ast) => {
            collect_expression_refs(x_ast, out);
            collect_expression_refs(y_ast, out);
        }
    }
}

pub(super) fn collect_expression_input_ids(refs: &ExpressionRefs) -> Vec<ChartItemId> {
    let mut input_ids = refs
        .item_refs
        .iter()
        .map(|item_ref| item_ref.id)
        .chain(
            refs.series_refs
                .iter()
                .filter_map(|series_ref| match &series_ref.target {
                    ExpressionObjectTarget::ItemRef(id) => Some(*id),
                    ExpressionObjectTarget::AbsolutePath(_) => None,
                }),
        )
        .chain(
            refs.scalar_refs
                .iter()
                .filter_map(|scalar_ref| match &scalar_ref.target {
                    ExpressionObjectTarget::ItemRef(id) => Some(*id),
                    ExpressionObjectTarget::AbsolutePath(_) => None,
                }),
        )
        .collect::<Vec<_>>();
    input_ids.sort_by_key(|id| id.0);
    input_ids.dedup();
    input_ids
}
