use crate::error::AppError;

#[derive(Debug, Clone)]
pub enum Command {
    Increment(usize),
    Decrement(usize),
    Seek(usize),
    Noop,
}

pub struct CommandState {
    pub command_buffer: String,
    pub cursor: usize,
    pub last_command: Command,
}

impl CommandState {
    pub fn parse_command(&mut self) -> Result<Command, AppError> {
        let command = self.command_buffer.trim();
        if command.is_empty() {
            return Ok(Command::Noop);
        }
        let first_symbol = command.chars().next().expect("Command should not be empty");
        match first_symbol {
            '+' => {
                let increment: usize = command[1..].trim().parse().map_err(|_| {
                    AppError::InvalidCommand(format!("Invalid increment value: {}", command))
                })?;
                self.last_command = Command::Increment(increment);
                Ok(Command::Increment(increment))
            }
            '-' => {
                let decrement: usize = command[1..].trim().parse().map_err(|_| {
                    AppError::InvalidCommand(format!("Invalid decrement value: {}", command))
                })?;
                self.last_command = Command::Decrement(decrement);
                Ok(Command::Decrement(decrement))
            }
            _ => {
                let seek: usize = command.parse().map_err(|_| {
                    AppError::InvalidCommand(format!("Invalid seek value: {}", command))
                })?;
                self.last_command = Command::Seek(seek);
                Ok(Command::Seek(seek))
            }
        }
    }
}
