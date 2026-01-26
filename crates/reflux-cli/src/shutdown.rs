use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Condvar, Mutex};
use std::time::Duration;

/// A shutdown signal that supports interruptible waits.
///
/// Unlike `thread::sleep()`, waits on this signal can be interrupted
/// immediately when shutdown is triggered.
pub struct ShutdownSignal {
    shutdown: AtomicBool,
    condvar: Condvar,
    mutex: Mutex<()>,
}

impl ShutdownSignal {
    /// Create a new shutdown signal in the non-shutdown state.
    pub fn new() -> Self {
        Self {
            shutdown: AtomicBool::new(false),
            condvar: Condvar::new(),
            mutex: Mutex::new(()),
        }
    }

    /// Trigger the shutdown signal, waking all waiting threads.
    pub fn trigger(&self) {
        self.shutdown.store(true, Ordering::SeqCst);
        self.condvar.notify_all();
    }

    /// Check if shutdown has been triggered.
    pub fn is_shutdown(&self) -> bool {
        self.shutdown.load(Ordering::SeqCst)
    }

    /// Wait for the specified duration or until shutdown is triggered.
    ///
    /// Returns `true` if shutdown was triggered, `false` if the wait completed normally.
    pub fn wait(&self, duration: Duration) -> bool {
        if self.is_shutdown() {
            return true;
        }

        let guard = self.mutex.lock().unwrap();
        let result = self
            .condvar
            .wait_timeout_while(guard, duration, |_| !self.is_shutdown());

        match result {
            Ok((_, timeout_result)) => {
                // If we didn't timeout, it means shutdown was triggered
                !timeout_result.timed_out()
            }
            Err(_) => {
                // Mutex poisoned, treat as shutdown
                true
            }
        }
    }

    /// Get a reference to the underlying AtomicBool for compatibility
    /// with code that expects `&AtomicBool`.
    pub fn as_atomic(&self) -> &AtomicBool {
        &self.shutdown
    }
}

impl Default for ShutdownSignal {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::thread;
    use std::time::Instant;

    #[test]
    fn test_initial_state() {
        let signal = ShutdownSignal::new();
        assert!(!signal.is_shutdown());
    }

    #[test]
    fn test_trigger() {
        let signal = ShutdownSignal::new();
        signal.trigger();
        assert!(signal.is_shutdown());
    }

    #[test]
    fn test_wait_timeout() {
        let signal = ShutdownSignal::new();
        let start = Instant::now();
        let interrupted = signal.wait(Duration::from_millis(50));
        let elapsed = start.elapsed();

        assert!(!interrupted);
        assert!(elapsed >= Duration::from_millis(50));
        assert!(elapsed < Duration::from_millis(200));
    }

    #[test]
    fn test_wait_interrupted() {
        let signal = Arc::new(ShutdownSignal::new());
        let signal_clone = Arc::clone(&signal);

        let handle = thread::spawn(move || {
            let start = Instant::now();
            let interrupted = signal_clone.wait(Duration::from_secs(10));
            (interrupted, start.elapsed())
        });

        // Give the thread time to start waiting
        thread::sleep(Duration::from_millis(50));

        // Trigger shutdown
        signal.trigger();

        let (interrupted, elapsed) = handle.join().unwrap();
        assert!(interrupted);
        assert!(elapsed < Duration::from_secs(1));
    }

    #[test]
    fn test_wait_already_shutdown() {
        let signal = ShutdownSignal::new();
        signal.trigger();

        let start = Instant::now();
        let interrupted = signal.wait(Duration::from_secs(10));
        let elapsed = start.elapsed();

        assert!(interrupted);
        assert!(elapsed < Duration::from_millis(100));
    }

    #[test]
    fn test_as_atomic() {
        let signal = ShutdownSignal::new();
        let atomic = signal.as_atomic();

        assert!(!atomic.load(Ordering::SeqCst));
        signal.trigger();
        assert!(atomic.load(Ordering::SeqCst));
    }
}
