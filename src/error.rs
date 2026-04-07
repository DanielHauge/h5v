use std::fmt::Display;
use std::io::Write;
use std::sync::mpsc::SendError;

#[derive(Debug)]
pub enum AppError {
    FileError(String),
    Io(std::io::Error),
    Hdf5(hdf5_metno::Error),
    ChannelError(String),
    ClipboardError(String),
    InvalidCommand(String),
}

impl Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AppError::Io(err) => write!(f, "IO Error: {}", err),
            AppError::Hdf5(err) => write!(f, "HDF5 Error: {}", err),
            AppError::ChannelError(c) => write!(f, "Channel Error: {}", c),
            AppError::ClipboardError(msg) => write!(f, "Clipboard Error: {}", msg),
            AppError::InvalidCommand(cmd) => write!(f, "Invalid Command: {}", cmd),
            AppError::FileError(x) => write!(f, "File error: {x}"),
        }
    }
}

impl From<std::io::Error> for AppError {
    fn from(err: std::io::Error) -> Self {
        AppError::Io(err)
    }
}

impl From<hdf5_metno::Error> for AppError {
    fn from(err: hdf5_metno::Error) -> Self {
        AppError::Hdf5(err)
    }
}

impl<T> From<SendError<T>> for AppError {
    fn from(x: SendError<T>) -> Self {
        AppError::ChannelError(format!("Failed to send message: {}", x))
    }
}

pub fn log_error(str: impl Display) {
    // TODO: Maybe fallback logpath with "dirs"
    let log_path_opt = option_env!("H5V_LOGPATH");
    if let Some(log_path) = log_path_opt {
        if let Ok(mut log_file) = std::fs::File::open(log_path) {
            // write!(log_file, "{}", str);
            let _ = write!(log_file, "{}", str);
        }
    }
}

#[allow(dead_code)]
pub fn log_error_panic(str: impl Display) {
    log_error(&str);
    panic!("{}", str);
}
