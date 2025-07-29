use std::{fmt::Display, sync::mpsc::SendError};

#[derive(Debug)]
pub enum AppError {
    Io(std::io::Error),
    Hdf5(hdf5_metno::Error),
    ChannelError(String),
    ClipboardError(String),
    InvalidCommand(String),
    InternalError(String),
}

impl Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AppError::Io(err) => write!(f, "IO Error: {}", err),
            AppError::Hdf5(err) => write!(f, "HDF5 Error: {}", err),
            AppError::ChannelError(c) => write!(f, "Channel Error: {}", c),
            AppError::ClipboardError(msg) => write!(f, "Clipboard Error: {}", msg),
            AppError::InvalidCommand(cmd) => write!(f, "Invalid Command: {}", cmd),
            AppError::InternalError(msg) => write!(f, "Internal Error: {}", msg),
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
