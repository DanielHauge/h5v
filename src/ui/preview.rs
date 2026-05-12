use std::{fs, path::Path, time::SystemTime};

use ratatui::{
    layout::{Alignment, Constraint, Rect},
    style::{Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Cell, Paragraph, Row, Table, Wrap},
    Frame,
};
use time::{format_description::well_known::Rfc3339, OffsetDateTime, UtcOffset};

#[cfg(unix)]
use std::{
    ffi::CStr,
    mem::MaybeUninit,
    os::unix::fs::{MetadataExt, PermissionsExt},
    ptr,
};

use super::{
    image_preview::render_img,
    preview_chart::{render_chart_preview, render_precomputed_chart_preview},
    state::AppState,
    std_comp_render::{
        render_empty_dataset, render_error, render_string, render_unsupported_rendering,
    },
};
use crate::{
    configure,
    error::AppError,
    h5f::{read_opaque_dataset_preview, read_string_dataset_preview, Encoding, H5FNode, Node},
    sprint_typedesc::sprint_type_schema,
};

fn format_size(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KiB", "MiB", "GiB", "TiB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }

    if unit == 0 {
        format!("{bytes} {}", UNITS[unit])
    } else {
        format!("{value:.2} {} ({bytes} B)", UNITS[unit])
    }
}

fn format_timestamp(time: SystemTime) -> Option<String> {
    let date_time = OffsetDateTime::from(time);
    let date_time = UtcOffset::current_local_offset()
        .map(|offset| date_time.to_offset(offset))
        .unwrap_or(date_time);
    date_time.format(&Rfc3339).ok()
}

#[cfg(unix)]
fn format_permissions(metadata: &fs::Metadata) -> String {
    let mode = metadata.permissions().mode();
    let bits = [
        (0o400, 'r'),
        (0o200, 'w'),
        (0o100, 'x'),
        (0o040, 'r'),
        (0o020, 'w'),
        (0o010, 'x'),
        (0o004, 'r'),
        (0o002, 'w'),
        (0o001, 'x'),
    ];
    let symbolic: String = bits
        .into_iter()
        .map(|(bit, ch)| if mode & bit != 0 { ch } else { '-' })
        .collect();
    format!("{symbolic} ({:o})", mode & 0o777)
}

#[cfg(not(unix))]
fn format_permissions(metadata: &fs::Metadata) -> String {
    if metadata.permissions().readonly() {
        "read-only".to_string()
    } else {
        "read-write".to_string()
    }
}

fn truncate_left(text: &str, offset: usize) -> String {
    text.chars().skip(offset).collect()
}

fn truncate_left_line(line: &Line<'static>, offset: usize) -> Line<'static> {
    if offset == 0 {
        return line.clone();
    }

    let mut remaining = offset;
    let mut spans = Vec::new();
    for span in &line.spans {
        let content = span.content.as_ref();
        let content_len = content.chars().count();
        if remaining >= content_len {
            remaining -= content_len;
            continue;
        }

        let truncated = if remaining == 0 {
            content.to_string()
        } else {
            truncate_left(content, remaining)
        };
        spans.push(Span::styled(truncated, span.style));
        remaining = 0;
    }

    Line::from(spans)
}

fn plain_file_value(value: impl Into<String>) -> Line<'static> {
    Line::from(value.into())
}

#[cfg(unix)]
fn lookup_posix_name_buffer_size(kind: libc::c_int) -> usize {
    let size = unsafe { libc::sysconf(kind) };
    if size <= 0 {
        1024
    } else {
        size as usize
    }
}

#[cfg(unix)]
fn lookup_user_name(uid: u32) -> Option<String> {
    let mut buffer_size = lookup_posix_name_buffer_size(libc::_SC_GETPW_R_SIZE_MAX);
    for _ in 0..4 {
        let mut passwd = MaybeUninit::<libc::passwd>::uninit();
        let mut result = ptr::null_mut();
        let mut buffer = vec![0_u8; buffer_size];
        let status = unsafe {
            libc::getpwuid_r(
                uid,
                passwd.as_mut_ptr(),
                buffer.as_mut_ptr().cast(),
                buffer.len(),
                &mut result,
            )
        };
        if status == libc::ERANGE {
            buffer_size *= 2;
            continue;
        }
        if status != 0 || result.is_null() {
            return None;
        }

        let passwd = unsafe { passwd.assume_init() };
        if passwd.pw_name.is_null() {
            return None;
        }

        return Some(
            unsafe { CStr::from_ptr(passwd.pw_name) }
                .to_string_lossy()
                .into_owned(),
        );
    }

    None
}

#[cfg(unix)]
fn lookup_group_name(gid: u32) -> Option<String> {
    let mut buffer_size = lookup_posix_name_buffer_size(libc::_SC_GETGR_R_SIZE_MAX);
    for _ in 0..4 {
        let mut group = MaybeUninit::<libc::group>::uninit();
        let mut result = ptr::null_mut();
        let mut buffer = vec![0_u8; buffer_size];
        let status = unsafe {
            libc::getgrgid_r(
                gid,
                group.as_mut_ptr(),
                buffer.as_mut_ptr().cast(),
                buffer.len(),
                &mut result,
            )
        };
        if status == libc::ERANGE {
            buffer_size *= 2;
            continue;
        }
        if status != 0 || result.is_null() {
            return None;
        }

        let group = unsafe { group.assume_init() };
        if group.gr_name.is_null() {
            return None;
        }

        return Some(
            unsafe { CStr::from_ptr(group.gr_name) }
                .to_string_lossy()
                .into_owned(),
        );
    }

    None
}

#[cfg(unix)]
fn format_posix_identity(value: u32, name: Option<String>) -> Line<'static> {
    let Some(name) = name else {
        return plain_file_value(value.to_string());
    };

    Line::from(vec![
        Span::raw(value.to_string()),
        Span::styled(
            format!(" ({name})"),
            Style::default().fg(configure::themed_color(|colors| colors.file.section_title)),
        ),
    ])
}

fn render_empty_group_preview(f: &mut Frame, area: &Rect) {
    let text = Text::from(vec![
        Line::from("This group is just chilling."),
        Line::from(""),
        Line::from("No preview expression lives here yet."),
        Line::from(""),
        Line::from("Add `H5V_PREVIEW_EXPR` if you want this pane"),
        Line::from("to wake up and draw a chart."),
        Line::from(""),
        Line::from("   (for now it is a cozy little folder void)"),
    ]);
    let paragraph = Paragraph::new(text)
        .style({
            let mut style =
                Style::default().fg(configure::themed_color(|colors| colors.text.primary));
            if configure::prefers_strong_text() {
                style = style.add_modifier(Modifier::BOLD);
            }
            style
        })
        .alignment(Alignment::Center)
        .wrap(Wrap { trim: true })
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(
                    Style::default()
                        .fg(configure::themed_color(|colors| colors.surface.break_line)),
                )
                .title(configure::configured_symbol(|symbols| {
                    symbols.title.empty_group
                }))
                .title_alignment(Alignment::Center),
        );
    f.render_widget(paragraph, *area);
}

fn render_file_preview(
    f: &mut Frame,
    area: &Rect,
    file: &hdf5_metno::File,
    selected_node: &mut H5FNode,
    state: &AppState,
) {
    let file_path = file.filename();
    let display_path = Path::new(&file_path);
    let metadata = match fs::metadata(display_path) {
        Ok(metadata) => metadata,
        Err(error) => {
            render_error(
                f,
                area,
                format!(
                    "Failed to read file metadata for '{}': {}",
                    file_path, error
                ),
            );
            return;
        }
    };

    let canonical_path = fs::canonicalize(display_path)
        .ok()
        .map(|path| path.display().to_string())
        .unwrap_or_else(|| file_path.clone());
    let file_name = display_path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(file_path.as_str())
        .to_string();
    let modified = metadata
        .modified()
        .ok()
        .and_then(format_timestamp)
        .unwrap_or_else(|| "unavailable".to_string());
    let created = metadata
        .created()
        .ok()
        .and_then(format_timestamp)
        .unwrap_or_else(|| "unavailable".to_string());
    let accessed = metadata
        .accessed()
        .ok()
        .and_then(format_timestamp)
        .unwrap_or_else(|| "unavailable".to_string());

    let mut rows = vec![
        ("file name".to_string(), plain_file_value(file_name)),
        (
            "filesystem path".to_string(),
            plain_file_value(file_path.clone()),
        ),
        (
            "canonical path".to_string(),
            plain_file_value(canonical_path),
        ),
        ("hdf5 root path".to_string(), plain_file_value(file.name())),
        (
            "open mode".to_string(),
            plain_file_value(if state.readonly {
                "read-only".to_string()
            } else {
                "read-write".to_string()
            }),
        ),
        (
            "path type".to_string(),
            plain_file_value(if state.file_watch.linked {
                "opened through a symlink".to_string()
            } else {
                "direct file path".to_string()
            }),
        ),
        (
            "file size".to_string(),
            plain_file_value(format_size(metadata.len())),
        ),
        ("modified".to_string(), plain_file_value(modified)),
        ("created".to_string(), plain_file_value(created)),
        ("accessed".to_string(), plain_file_value(accessed)),
        (
            "permissions".to_string(),
            plain_file_value(format_permissions(&metadata)),
        ),
    ];

    #[cfg(unix)]
    {
        rows.push((
            "owner uid".to_string(),
            format_posix_identity(metadata.uid(), lookup_user_name(metadata.uid())),
        ));
        rows.push((
            "group gid".to_string(),
            format_posix_identity(metadata.gid(), lookup_group_name(metadata.gid())),
        ));
        rows.push((
            "inode".to_string(),
            plain_file_value(metadata.ino().to_string()),
        ));
        rows.push((
            "hard links".to_string(),
            plain_file_value(metadata.nlink().to_string()),
        ));
    }

    let outer = Block::default()
        .title(configure::configured_symbol(|symbols| {
            symbols.title.file_metadata
        }))
        .title_style(
            Style::default()
                .fg(configure::themed_color(|colors| colors.surface.panel_title))
                .add_modifier(Modifier::BOLD),
        )
        .borders(Borders::ALL)
        .border_style(
            Style::default().fg(configure::themed_color(|colors| colors.surface.break_line)),
        )
        .style(Style::default().bg(configure::themed_color(|colors| colors.surface.bg)));
    let inner = outer.inner(*area);
    f.render_widget(outer, *area);

    if inner.height == 0 {
        return;
    }

    let visible_rows = inner.height as usize;
    let max_offset = rows.len().saturating_sub(visible_rows.max(1));
    selected_node.line_offset = selected_node.line_offset.min(max_offset);
    let offset = selected_node.line_offset;
    let col_offset = selected_node.col_offset.max(0) as usize;
    let label_width = rows
        .iter()
        .map(|(label, _)| label.len())
        .max()
        .unwrap_or(12)
        .max(12) as u16;

    let table_rows = rows
        .into_iter()
        .skip(offset)
        .take(visible_rows)
        .enumerate()
        .map(|(index, (label, value))| {
            let bg = if index % 2 == 0 {
                configure::themed_color(|colors| colors.surface.bg)
            } else {
                configure::themed_color(|colors| colors.surface.bg_val1)
            };
            Row::new(vec![
                Cell::from(label).style(
                    Style::default()
                        .fg(configure::themed_color(|colors| colors.file.label))
                        .add_modifier(Modifier::BOLD),
                ),
                Cell::from(truncate_left_line(&value, col_offset)).style({
                    let mut style =
                        Style::default().fg(configure::themed_color(|colors| colors.file.value));
                    if configure::prefers_strong_text() {
                        style = style.add_modifier(Modifier::BOLD);
                    }
                    style
                }),
            ])
            .style(Style::default().bg(bg))
        });

    let table = Table::new(
        table_rows,
        [Constraint::Length(label_width), Constraint::Min(10)],
    )
    .column_spacing(3)
    .block(
        Block::default()
            .title(" paths, timestamps, ownership, and access ")
            .title_style(
                Style::default().fg(configure::themed_color(|colors| colors.file.section_title)),
            ),
    );
    f.render_widget(table, inner);
}

#[cfg(test)]
mod tests {
    use super::truncate_left_line;
    use ratatui::{
        style::{Color, Style},
        text::{Line, Span},
    };

    #[test]
    fn truncate_left_line_preserves_remaining_styles() {
        let line = Line::from(vec![
            Span::raw("1000"),
            Span::styled(" (archie)", Style::default().fg(Color::Yellow)),
        ]);

        let truncated = truncate_left_line(&line, 5);

        assert_eq!(truncated.spans.len(), 1);
        assert_eq!(truncated.spans[0].content.as_ref(), "(archie)");
        assert_eq!(truncated.spans[0].style.fg, Some(Color::Yellow));
    }
}

fn compound_schema_preview_text(attr: &crate::h5f::DatasetMeta) -> String {
    let path = attr.virtual_path().unwrap_or(attr.display_name.as_str());
    format!(
        "Compound schema: {path}\n\n{}",
        sprint_type_schema(&attr.type_descriptor)
    )
}

pub fn render_preview(
    f: &mut Frame,
    area: &Rect,
    selected_node: &mut H5FNode,
    state: &mut AppState,
) {
    let area_inner = area.inner(ratatui::layout::Margin {
        horizontal: 2,
        vertical: 1,
    });
    let node = selected_node.node.clone();

    if let Node::File(file) = node {
        render_file_preview(f, &area_inner, &file, selected_node, state);
        return;
    }

    if let Node::Group(_, meta) = node {
        match meta.preview_expr.as_deref() {
            Some(expression) => {
                match state
                    .multi_chart
                    .evaluate_expression_preview(expression, state.file.as_ref())
                {
                    Ok(data_preview) => {
                        if let Err(e) = render_precomputed_chart_preview(
                            f,
                            &area_inner,
                            selected_node,
                            state,
                            data_preview,
                        ) {
                            render_error(f, &area_inner, format!("Render chart error: {}", e));
                        }
                    }
                    Err(e) => {
                        render_error(
                            f,
                            &area_inner,
                            format!("Error evaluating H5V_PREVIEW_EXPR: {}", e),
                        );
                    }
                }
            }
            None => render_empty_group_preview(f, &area_inner),
        }
        return;
    }

    if let Node::Dataset(dataset, attr) = node {
        if attr.is_empty() {
            render_empty_dataset(f, &area_inner);
            return;
        }
        if attr.is_compound_container() {
            render_string(
                f,
                &area_inner,
                selected_node,
                compound_schema_preview_text(&attr),
                None,
            );
            return;
        }
        if attr.is_opaque() {
            match read_opaque_dataset_preview(&dataset, &attr) {
                Ok(text) => render_string(f, &area_inner, selected_node, text, None),
                Err(e) => render_error(f, &area_inner, format!("Render opaque error: {}", e)),
            }
            return;
        }
        match &attr.image {
            Some(image_type) => {
                match render_img(image_type, f, &area_inner, selected_node, state) {
                    Ok(()) => {}
                    Err(e) => {
                        render_error(f, &area_inner, format!("Render img error: {}", e));
                    }
                }
            }
            None => {
                if attr.matrixable.is_none() {
                    match render_string_preview(f, &area_inner, selected_node) {
                        Ok(()) => {}
                        Err(e) => {
                            render_error(f, &area_inner, format!("Render string error: {}", e));
                        }
                    }
                } else {
                    match render_chart_preview(f, &area_inner, selected_node, state) {
                        Ok(()) => {}
                        Err(e) => {
                            render_error(f, &area_inner, format!("Render chart error: {}", e));
                        }
                    }
                }
            }
        }
    }
}

pub fn render_string_preview(
    f: &mut Frame,
    area: &Rect,
    node: &mut H5FNode,
) -> Result<(), AppError> {
    let selected_node = &node.node;
    let (dataset, meta) = match selected_node {
        Node::Dataset(ds, attr) => (ds, attr),
        _ => {
            render_unsupported_rendering(
                f,
                area,
                selected_node,
                "Selected node is not a dataset, cannot render string preview",
            );
            return Ok(());
        }
    };

    if meta.is_opaque() {
        match read_opaque_dataset_preview(dataset, meta) {
            Ok(text) => render_string(f, area, node, text, None),
            Err(e) => render_error(f, area, format!("Error: {}", e)),
        }
        return Ok(());
    }

    match meta.encoding {
        Encoding::LittleEndian => {
            render_unsupported_rendering(
                f,
                area,
                selected_node,
                "LittleEndian not supported for string data",
            );
        }
        Encoding::Unknown => {
            render_unsupported_rendering(
                f,
                area,
                selected_node,
                "Unknown encoding not supported for string data",
            );
        }
        Encoding::Ascii | Encoding::UTF8 | Encoding::UTF8Fixed | Encoding::AsciiFixed => {
            match read_string_dataset_preview(dataset, &meta.encoding) {
                Ok(x) => render_string(f, area, node, x, meta.hl.clone()),
                Err(e) => render_error(f, area, format!("Error: {}", e)),
            }
        }
    }
    Ok(())
}

pub fn preview_text_for_compound_schema(meta: &crate::h5f::DatasetMeta) -> Option<String> {
    meta.is_compound_container()
        .then(|| compound_schema_preview_text(meta))
}
