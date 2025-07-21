use crate::error::AppError;

pub enum Command {
    Increment(usize),
    Decrement(usize),
    Seek(usize),
}

pub struct CommandState {
    pub command_buffer: String,
    pub cursor: usize,
    pub last_command: Option<Command>,
}

impl CommandState {
    pub fn parse_command(&mut self) -> Result<Command, AppError> {
        let command = self.command_buffer.trim();
        if command.is_empty() {
            return Err(AppError::InvalidCommand("Empty command".to_string()));
        }
        let first_symbol = command.chars().next().expect("Command should not be empty");
        match first_symbol {
            '+' => {
                let increment: usize = command[1..].trim().parse().map_err(|_| {
                    AppError::InvalidCommand(format!("Invalid increment value: {}", command))
                })?;
                self.last_command = Some(Command::Increment(increment));
                Ok(Command::Increment(increment))
            }
            '-' => {
                let decrement: usize = command[1..].trim().parse().map_err(|_| {
                    AppError::InvalidCommand(format!("Invalid decrement value: {}", command))
                })?;
                self.last_command = Some(Command::Decrement(decrement));
                Ok(Command::Decrement(decrement))
            }
            _ => {
                let seek: usize = command.parse().map_err(|_| {
                    AppError::InvalidCommand(format!("Invalid seek value: {}", command))
                })?;
                self.last_command = Some(Command::Seek(seek));
                Ok(Command::Seek(seek))
            }
        }
    }
}
