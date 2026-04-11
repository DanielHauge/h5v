pub fn image_capable_terminal() -> bool {
    if std::env::var("KITTY_WINDOW_ID").is_ok() {
        return true;
    }
    if std::env::var("TERM_PROGRAM")
        .map(|v| v == "iTerm.app")
        .unwrap_or(false)
    {
        return true;
    }
    if let Ok(term) = std::env::var("TERM") {
        if term.contains("sixel") {
            return true;
        }
    }
    false
}
