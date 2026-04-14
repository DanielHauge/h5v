use std::{
    fs::File,
    io::{stdout, Read, Write},
    process::Command,
};

use ratatui::crossterm::{
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    ExecutableCommand,
};
use uuid::Uuid;

use crate::{error::AppError, ui::state::AppState};

pub fn leave_h5v() -> Result<(), AppError> {
    stdout().execute(LeaveAlternateScreen)?;
    disable_raw_mode()?;
    ratatui::restore();
    Ok(())
}

pub fn reenter_h5v() -> Result<(), AppError> {
    stdout().execute(EnterAlternateScreen)?;
    enable_raw_mode()?;
    Ok(())
}

fn create_tmp_file() -> Result<(File, String), AppError> {
    let buf: [u8; 16] = *b"abcdefghijklmnop";
    let uuid = Uuid::new_v8(buf);
    let tmp_dir = dirs::cache_dir()
        .unwrap_or_default()
        .to_str()
        .unwrap()
        .to_string();
    let tmp_file_path = format!("{tmp_dir}/h5v_edit_{uuid}");
    let file = File::create(&tmp_file_path)?;
    Ok((file, tmp_file_path))
}

pub fn perform_edit(state: &mut AppState<'_>, content: String) -> Result<String, AppError> {
    leave_h5v()?;
    let edit_pause = state.edit_pause.write().unwrap();
    let (mut file, path) = create_tmp_file()?;
    file.write_all(&content.into_bytes()).unwrap();
    drop(file);

    let editor = option_env!("EDITOR").unwrap_or("vi");
    let editor_proc = Command::new(editor).arg(&path).spawn();
    editor_proc.unwrap().wait_with_output().unwrap();
    let mut new_content = String::new();

    let mut file = File::open(path).unwrap();
    file.read_to_string(&mut new_content).unwrap();
    let new_content = new_content.trim().to_string();
    drop(edit_pause);
    reenter_h5v()?;
    Ok(new_content)
}
