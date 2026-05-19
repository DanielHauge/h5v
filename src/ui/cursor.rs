use ratatui::{layout::Position, style::Modifier, Frame};

const BLINK_MODIFIERS: Modifier = Modifier::SLOW_BLINK.union(Modifier::RAPID_BLINK);

pub(crate) fn set_input_cursor(frame: &mut Frame<'_>, position: Position) {
    if cfg!(target_os = "windows") {
        if let Some(cell) = frame.buffer_mut().cell_mut(position) {
            cell.modifier.insert(Modifier::REVERSED);
        }
    } else {
        frame.set_cursor_position(position);
    }
}

pub(crate) fn strip_blink_modifiers(frame: &mut Frame<'_>) {
    for cell in &mut frame.buffer_mut().content {
        cell.modifier.remove(BLINK_MODIFIERS);
    }
}

#[cfg(test)]
mod tests {
    use ratatui::{
        backend::TestBackend,
        style::Modifier,
        widgets::{Block, Widget},
        Terminal,
    };

    use super::strip_blink_modifiers;

    #[test]
    fn strip_blink_modifiers_keeps_other_modifiers() {
        let backend = TestBackend::new(2, 1);
        let mut terminal = Terminal::new(backend).expect("terminal");

        let frame = terminal
            .draw(|frame| {
                Block::new().render(frame.area(), frame.buffer_mut());
                frame
                    .buffer_mut()
                    .cell_mut((0, 0))
                    .expect("cell")
                    .modifier
                    .insert(Modifier::BOLD | Modifier::SLOW_BLINK | Modifier::RAPID_BLINK);
                strip_blink_modifiers(frame);
            })
            .expect("draw");

        let modifier = frame.buffer.cell((0, 0)).expect("cell").modifier;
        assert!(modifier.contains(Modifier::BOLD));
        assert!(!modifier.intersects(Modifier::SLOW_BLINK | Modifier::RAPID_BLINK));
    }
}
