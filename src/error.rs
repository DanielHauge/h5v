use std::fmt::Display;
use std::sync::mpsc::SendError;
use std::sync::PoisonError;

use crate::configure::errors::ConfigureErrors;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FixedStringKind {
    Ascii,
    Unicode,
}

impl Display for FixedStringKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FixedStringKind::Ascii => write!(f, "FixedAscii"),
            FixedStringKind::Unicode => write!(f, "FixedUnicode"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FixedStringOverflow {
    pub kind: FixedStringKind,
    pub current_size: usize,
    pub required_size: usize,
}

impl Display for FixedStringOverflow {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} value requires {} bytes but current fixed size is {}",
            self.kind, self.required_size, self.current_size
        )
    }
}

#[derive(Debug)]
pub enum AppError {
    FileError(String),
    LuaError(ConfigureErrors),
    Io(std::io::Error),
    Hdf5(hdf5_metno::Error),
    ChannelError(String),
    ClipboardError(String),
    InvalidCommand(String),
    EditError(String),
    EditWarning(String),
    FixedStringOverflow(FixedStringOverflow),
    ChildNotFound(String),
    PoisonedLockError(String),
    DrawingError(String),
}

impl From<ConfigureErrors> for AppError {
    fn from(value: ConfigureErrors) -> Self {
        AppError::LuaError(value)
    }
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
            AppError::EditError(x) => write!(f, "Edit error: {x}"),
            AppError::ChildNotFound(x) => write!(f, "Child not found: {x}"),
            AppError::PoisonedLockError(x) => write!(f, "Poisoned lock error: {x}"),
            AppError::EditWarning(x) => write!(f, "Edit warning: {x}"),
            AppError::FixedStringOverflow(x) => write!(f, "Edit error: {x}"),
            AppError::DrawingError(x) => write!(f, "Drawing error: {x}"),
            AppError::LuaError(x) => write!(f, "Lua error: {x}"),
        }
    }
}

// impl<T> From<DrawingAreaError<T>> for AppError {
//     fn from(err: DrawingAreaError<T>) -> Self {
//         AppError::DrawingError(format!("Drawing area error: {}", err))
//     }
// }

impl<T> From<PoisonError<T>> for AppError {
    fn from(err: PoisonError<T>) -> Self {
        AppError::PoisonedLockError(format!("Poisoned lock error: {}", err))
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
    crate::logging::log_error("error", str.to_string());
}
