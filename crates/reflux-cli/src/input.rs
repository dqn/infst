use crate::shutdown::ShutdownSignal;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::Duration;
use tracing::debug;

/// Spawn a thread that monitors keyboard input for shutdown keys (Esc, q, Q).
///
/// The thread polls for keyboard events and triggers shutdown when:
/// - Esc key is pressed
/// - 'q' or 'Q' key is pressed
/// - Ctrl+C is pressed (as backup to ctrlc handler)
///
/// Returns a JoinHandle that can be used to wait for the thread to finish.
pub fn spawn_keyboard_monitor(shutdown: Arc<ShutdownSignal>) -> JoinHandle<()> {
    thread::spawn(move || {
        debug!("Keyboard monitor started");

        while !shutdown.is_shutdown() {
            // Poll for events with a timeout to allow checking shutdown state
            if event::poll(Duration::from_millis(100)).unwrap_or(false)
                && let Ok(Event::Key(key_event)) = event::read()
                && should_shutdown(&key_event)
            {
                debug!("Shutdown key pressed: {:?}", key_event.code);
                shutdown.trigger();
                break;
            }
        }

        debug!("Keyboard monitor stopped");
    })
}

/// Check if the key event should trigger shutdown.
fn should_shutdown(event: &KeyEvent) -> bool {
    match event.code {
        KeyCode::Esc => true,
        KeyCode::Char('q') | KeyCode::Char('Q') => true,
        KeyCode::Char('c') if event.modifiers.contains(KeyModifiers::CONTROL) => true,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_should_shutdown_esc() {
        let event = KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE);
        assert!(should_shutdown(&event));
    }

    #[test]
    fn test_should_shutdown_q() {
        let event = KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE);
        assert!(should_shutdown(&event));
    }

    #[test]
    fn test_should_shutdown_q_upper() {
        let event = KeyEvent::new(KeyCode::Char('Q'), KeyModifiers::SHIFT);
        assert!(should_shutdown(&event));
    }

    #[test]
    fn test_should_shutdown_ctrl_c() {
        let event = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
        assert!(should_shutdown(&event));
    }

    #[test]
    fn test_should_not_shutdown_other_keys() {
        let event = KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE);
        assert!(!should_shutdown(&event));

        let event = KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE);
        assert!(!should_shutdown(&event));

        let event = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::NONE);
        assert!(!should_shutdown(&event));
    }
}
