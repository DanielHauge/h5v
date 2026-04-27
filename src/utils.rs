pub fn image_capable_terminal() -> bool {
    std::env::var("KITTY_WINDOW_ID").is_ok()
        || std::env::var("TERM_PROGRAM").is_ok_and(|v| v == "iTerm.app")
        || std::env::var("TERM").is_ok_and(|term| term.contains("sixel"))
}
