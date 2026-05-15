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
    LoadRef(ExpressionLoadRef),
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
    LoadRef(ExpressionLoadRef),
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
    ItemRef(ExpressionItemTarget),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(super) enum ExpressionItemTarget {
    Id(ChartItemId),
    Name(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(super) struct ExpressionItemSlice {
    pub(super) start: usize,
    pub(super) end: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(super) struct ExpressionItemRef {
    pub(super) target: ExpressionItemTarget,
    pub(super) slice: Option<ExpressionItemSlice>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(super) struct ExpressionLoadRef {
    pub(super) target: ExpressionObjectTarget,
    pub(super) attr_name: Option<String>,
    pub(super) selectors: Option<Vec<ExpressionDatasetSelector>>,
}

pub(super) type ExpressionSeriesRef = ExpressionLoadRef;
pub(super) type ExpressionScalarRef = ExpressionLoadRef;

impl ExpressionObjectTarget {
    pub(super) fn render(&self) -> String {
        match self {
            ExpressionObjectTarget::AbsolutePath(path) => path.clone(),
            ExpressionObjectTarget::ItemRef(target) => match target {
                ExpressionItemTarget::Id(id) => format!("${}", id.0),
                ExpressionItemTarget::Name(name) => format!("${name}"),
            },
        }
    }
}

impl ExpressionItemRef {
    pub(super) fn render(&self) -> String {
        match &self.slice {
            Some(slice) => format!("{}[{}..{}]", self.target.render(), slice.start, slice.end),
            None => self.target.render(),
        }
    }
}

impl ExpressionItemTarget {
    pub(super) fn render(&self) -> String {
        match self {
            ExpressionItemTarget::Id(id) => format!("${}", id.0),
            ExpressionItemTarget::Name(name) => format!("${name}"),
        }
    }
}

impl ExpressionLoadRef {
    pub(super) fn render(&self) -> String {
        let base = match &self.attr_name {
            Some(attr_name) => format!("load({}:{attr_name})", self.target.render()),
            None => format!("load({})", self.target.render()),
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

    pub(super) fn to_series_preview_selection(
        &self,
        shape: &[usize],
    ) -> Result<PreviewSelection, String> {
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
                        "Reference {reference} needs an explicit selector like load(/path)[..,0] for rank-{} arrays",
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
            'a'..='z' | 'A'..='Z' | '_' => {
                let mut ident = String::new();
                while let Some(next) = chars.peek() {
                    if next.is_ascii_alphanumeric() || *next == '_' {
                        ident.push(*next);
                        chars.next();
                    } else {
                        break;
                    }
                }
                match ident.as_str() {
                    "load" => tokens.push(ExpressionToken::LoadRef(parse_expression_load_ref(
                        &mut chars,
                    )?)),
                    _ => {
                        return Err(format!(
                            "Unsupported function '{ident}' in expression. Use load(...), $id item references, numbers, + - * /, commas, and parentheses"
                        ))
                    }
                }
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
                    "Unsupported character '{}' in expression. Use $id item references, load(...) references, numbers, + - * /, commas, and parentheses",
                    other
                ));
            }
        }
    }

    Ok(tokens)
}

fn parse_expression_item_target(
    chars: &mut std::iter::Peekable<std::str::Chars<'_>>,
) -> Result<ExpressionItemTarget, String> {
    let mut target = String::new();
    while let Some(next) = chars.peek() {
        if next.is_ascii_alphanumeric() || *next == '_' {
            target.push(*next);
            chars.next();
        } else {
            break;
        }
    }
    if target.is_empty() {
        return Err("Expected digits or identifier after '$' item reference".to_string());
    }
    if target.chars().all(|ch| ch.is_ascii_digit()) {
        let id = target
            .parse::<u64>()
            .map_err(|_| format!("Invalid chart item reference '${target}'"))?;
        return Ok(ExpressionItemTarget::Id(ChartItemId(id)));
    }
    let first = target.chars().next().unwrap_or_default();
    if !first.is_ascii_alphabetic() && first != '_' {
        return Err(format!(
            "Named chart item reference '${target}' must start with a letter or '_'"
        ));
    }
    Ok(ExpressionItemTarget::Name(target))
}

pub(super) fn parse_expression_item_ref(
    chars: &mut std::iter::Peekable<std::str::Chars<'_>>,
) -> Result<ExpressionItemRef, String> {
    let target = parse_expression_item_target(chars)?;
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
                "Chart item reference '{}[{spec}' is missing a closing ']'",
                target.render()
            ));
        }
        Some(parse_expression_item_slice(&target, &spec)?)
    } else {
        None
    };
    Ok(ExpressionItemRef { target, slice })
}

fn parse_expression_item_slice(
    target: &ExpressionItemTarget,
    spec: &str,
) -> Result<ExpressionItemSlice, String> {
    let Some((start, end)) = spec.split_once("..") else {
        return Err(format!(
            "Chart item reference '{}[{spec}]' must use a slice like [0..5]",
            target.render()
        ));
    };
    let start = start.trim().parse::<usize>().map_err(|_| {
        format!(
            "Chart item reference '{}[{spec}]' has invalid slice start '{}'",
            target.render(),
            start.trim()
        )
    })?;
    let end = end.trim().parse::<usize>().map_err(|_| {
        format!(
            "Chart item reference '{}[{spec}]' has invalid slice end '{}'",
            target.render(),
            end.trim()
        )
    })?;
    if end <= start {
        return Err(format!(
            "Chart item reference '{}[{spec}]' must use an increasing slice",
            target.render()
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

fn skip_expression_call_whitespace(chars: &mut std::iter::Peekable<std::str::Chars<'_>>) {
    while matches!(chars.peek(), Some(next) if next.is_whitespace()) {
        chars.next();
    }
}

fn parse_expression_object_target(
    chars: &mut std::iter::Peekable<std::str::Chars<'_>>,
    function_name: &str,
) -> Result<ExpressionObjectTarget, String> {
    match chars.peek().copied() {
        Some('/') => Ok(ExpressionObjectTarget::AbsolutePath(
            parse_expression_absolute_path(chars)?,
        )),
        _ => Err(match function_name {
            "load" => "Data references must use load(/group/dataset) or load(/group/dataset:ATTR)"
                .to_string(),
            _ => "Invalid expression reference".to_string(),
        }),
    }
}

pub(super) fn parse_expression_load_ref(
    chars: &mut std::iter::Peekable<std::str::Chars<'_>>,
) -> Result<ExpressionLoadRef, String> {
    skip_expression_call_whitespace(chars);
    if chars.next() != Some('(') {
        return Err("Data references must use load(...)".to_string());
    }
    skip_expression_call_whitespace(chars);
    let target = parse_expression_object_target(chars, "load")?;
    let attr_name = if chars.peek() == Some(&':') {
        chars.next();
        let attr_name = parse_expression_attribute_name(chars);
        if attr_name.is_empty() {
            return Err(format!(
                "Expected an attribute name after '{}:' in load reference",
                target.render()
            ));
        }
        Some(attr_name)
    } else {
        None
    };

    let mut load_ref = ExpressionLoadRef {
        target,
        attr_name,
        selectors: None,
    };
    skip_expression_call_whitespace(chars);
    if chars.next() != Some(')') {
        return Err(format!(
            "Load reference {} is missing a closing ')'",
            load_ref.render().trim_end_matches(')')
        ));
    }
    if chars.peek() == Some(&'[') {
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
                "Load reference '{}[{spec}' is missing a closing ']'",
                load_ref.render()
            ));
        }
        load_ref.selectors = Some(parse_expression_dataset_selectors(
            &load_ref.render(),
            &spec,
        )?);
    }
    Ok(load_ref)
}

pub(super) fn parse_expression_series_ref(
    chars: &mut std::iter::Peekable<std::str::Chars<'_>>,
) -> Result<ExpressionLoadRef, String> {
    parse_expression_load_ref(chars)
}

pub(super) fn parse_expression_scalar_ref(
    chars: &mut std::iter::Peekable<std::str::Chars<'_>>,
) -> Result<ExpressionLoadRef, String> {
    parse_expression_load_ref(chars)
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
            ExpressionToken::LoadRef(load_ref) => {
                *pos += 1;
                Ok(ExpressionAst::LoadRef(load_ref.clone()))
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
    pub(super) load_refs: Vec<ExpressionLoadRef>,
}

fn collect_expression_refs(expr: &ExpressionAst, out: &mut ExpressionRefs) {
    match expr {
        ExpressionAst::Number(_) => {}
        ExpressionAst::ItemRef(item_ref) => out.item_refs.push(item_ref.clone()),
        ExpressionAst::LoadRef(load_ref) => out.load_refs.push(load_ref.clone()),
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
        .filter_map(|item_ref| match item_ref.target {
            ExpressionItemTarget::Id(id) => Some(id),
            ExpressionItemTarget::Name(_) => None,
        })
        .chain(
            refs.load_refs
                .iter()
                .filter_map(|load_ref| match &load_ref.target {
                    ExpressionObjectTarget::ItemRef(ExpressionItemTarget::Id(id)) => Some(*id),
                    ExpressionObjectTarget::ItemRef(ExpressionItemTarget::Name(_)) => None,
                    ExpressionObjectTarget::AbsolutePath(_) => None,
                }),
        )
        .collect::<Vec<_>>();
    input_ids.sort_by_key(|id| id.0);
    input_ids.dedup();
    input_ids
}
