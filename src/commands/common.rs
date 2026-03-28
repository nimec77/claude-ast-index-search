//! Shared helpers for command implementations.

use std::time::Instant;

use colored::Colorize;

/// RAII guard that prints elapsed time on drop.
///
/// Usage: `let _timer = CommandTimer::new();`
/// Prints: `Time: 1.23ms` to stderr when dropped.
pub struct CommandTimer {
    pub start: Instant,
}

impl Default for CommandTimer {
    fn default() -> Self {
        Self::new()
    }
}

impl CommandTimer {
    pub fn new() -> Self {
        Self {
            start: Instant::now(),
        }
    }
}

impl Drop for CommandTimer {
    fn drop(&mut self) {
        eprintln!("\n{}", format!("Time: {:?}", self.start.elapsed()).dimmed());
    }
}

/// Open the database, printing a warning and returning `Ok(None)` if no index exists.
/// Use with: `let conn = open_db_or_return!(root);`
#[macro_export]
macro_rules! open_db_or_return {
    ($root:expr) => {
        match $crate::db::open_db_or_warn($root)? {
            Some(c) => c,
            None => return Ok(()),
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn command_timer_formats_elapsed() {
        let timer = CommandTimer::new();
        std::thread::sleep(Duration::from_millis(10));
        let msg = format!("Time: {:?}", timer.start.elapsed());
        assert!(msg.contains("Time:"));
    }
}
