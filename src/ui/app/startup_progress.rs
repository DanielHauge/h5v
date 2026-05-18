use std::{
    io::{stdout, Write},
    sync::{
        atomic::{AtomicBool, Ordering},
        LazyLock, Mutex,
    },
};

use ratatui::crossterm::{
    cursor::MoveTo,
    style::Print,
    terminal::{self, Clear, ClearType},
    QueueableCommand,
};

static STARTUP_PROGRESS_ENABLED: AtomicBool = AtomicBool::new(false);
static STARTUP_PROGRESS_RENDER_LOCK: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

pub(super) fn set_startup_progress_enabled(enabled: bool) {
    STARTUP_PROGRESS_ENABLED.store(enabled, Ordering::Relaxed);
}

pub(crate) fn render_startup_progress(stage: &str, detail: Option<&str>) {
    if !STARTUP_PROGRESS_ENABLED.load(Ordering::Relaxed) {
        return;
    }

    let Ok(_guard) = STARTUP_PROGRESS_RENDER_LOCK.lock() else {
        return;
    };

    let (width, height) = terminal::size().unwrap_or((80, 24));
    let lines = startup_progress_lines(stage, detail);
    let start_y = height.saturating_sub(lines.len() as u16) / 2;
    let mut out = stdout();

    if out.queue(Clear(ClearType::All)).is_err() {
        return;
    }

    for (index, line) in lines.iter().enumerate() {
        let y = start_y.saturating_add(index as u16);
        let rendered = truncate_for_width(line, width);
        if out
            .queue(MoveTo(centered_x(width, &rendered), y))
            .and_then(|out| out.queue(Print(rendered)))
            .is_err()
        {
            return;
        }
    }

    let _ = out.flush();
}

fn startup_progress_lines(stage: &str, detail: Option<&str>) -> Vec<String> {
    let mut lines = vec![
        "h5v".to_string(),
        "Starting up...".to_string(),
        String::new(),
        stage.trim().to_string(),
    ];
    if let Some(detail) = detail.map(str::trim).filter(|detail| !detail.is_empty()) {
        lines.push(detail.to_string());
    }
    lines.push(String::new());
    lines.push("Loading configuration, plugins, and file state.".to_string());
    lines
}

fn centered_x(width: u16, text: &str) -> u16 {
    let text_width = text.chars().count().min(width as usize) as u16;
    width.saturating_sub(text_width) / 2
}

fn truncate_for_width(text: &str, width: u16) -> String {
    text.chars().take(width as usize).collect()
}

#[cfg(test)]
mod tests {
    use super::startup_progress_lines;

    #[test]
    fn startup_progress_lines_include_detail_when_present() {
        let lines = startup_progress_lines("Cloning plugin...", Some("owner/repo"));
        assert!(lines.iter().any(|line| line == "Cloning plugin..."));
        assert!(lines.iter().any(|line| line == "owner/repo"));
    }

    #[test]
    fn startup_progress_lines_skip_blank_detail() {
        let lines = startup_progress_lines("Loading configuration...", Some("   "));
        assert_eq!(
            lines,
            vec![
                "h5v",
                "Starting up...",
                "",
                "Loading configuration...",
                "",
                "Loading configuration, plugins, and file state.",
            ]
        );
    }
}
