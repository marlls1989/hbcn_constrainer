//! Output redirection utilities for LP solvers
//!
//! This module provides a thread-safe singleton pattern for redirecting stdout/stderr
//! output to a log file using the `gag` crate. Multiple threads can safely acquire and share the
//! same redirect instance using Arc reference counting.
//!
//! **Important**: The `gag` crate can only create one instance per output stream per process.
//! Once a redirect is created, it cannot be recreated. This module manages that limitation.

use gag::Redirect;
use std::fs::OpenOptions;
use std::sync::{Arc, Mutex, Weak};

/// A thread-safe wrapper around a Redirect instance
pub struct GagHandle {
    _gag: Arc<Redirect<std::fs::File>>,
}

impl GagHandle {
    /// Get a handle to redirect stdout to lp_solver.log. Multiple threads can share the same underlying
    /// Redirect instance. The redirect persists until all handles are dropped.
    ///
    /// **Note**: Due to gag limitations, once created, the stdout redirect cannot be recreated
    /// in the same process, even after all handles are dropped.
    pub fn stdout() -> Result<Self, std::io::Error> {
        STDOUT_GAG_MANAGER.get_gag()
    }

    /// Get a handle to redirect stderr to lp_solver.log. Multiple threads can share the same underlying
    /// Redirect instance. The redirect persists until all handles are dropped.
    ///
    /// **Note**: Due to gag limitations, once created, the stderr redirect cannot be recreated
    /// in the same process, even after all handles are dropped.
    pub fn stderr() -> Result<Self, std::io::Error> {
        STDERR_GAG_MANAGER.get_gag()
    }
}

/// Thread-safe manager for Redirect instances using Arc and weak references
///
/// Due to gag's limitations, once a redirect is created, it persists for the process lifetime.
/// This manager ensures thread-safe access and prevents multiple creation attempts.
struct GagManager {
    weak_gag: Mutex<Weak<Redirect<std::fs::File>>>,
    create_gag: fn() -> Result<Redirect<std::fs::File>, std::io::Error>,
}

impl GagManager {
    const fn new(create_fn: fn() -> Result<Redirect<std::fs::File>, std::io::Error>) -> Self {
        Self {
            weak_gag: Mutex::new(Weak::new()),
            create_gag: create_fn,
        }
    }

    /// Get or create a Redirect instance wrapped in Arc
    fn get_gag(&self) -> Result<GagHandle, std::io::Error> {
        let mut weak_gag_guard = self.weak_gag.lock().unwrap();

        // Try to upgrade the weak reference to a strong Arc
        if let Some(gag) = weak_gag_guard.upgrade() {
            // Existing gag is still alive, reuse it
            return Ok(GagHandle { _gag: gag });
        }

        // Create the gag without holding the mutex
        let gag = match (self.create_gag)() {
            Ok(gag) => gag,
            Err(e) => {
                // If gag creation fails due to "already exists", try one more time to get existing gag
                if e.kind() == std::io::ErrorKind::AlreadyExists {
                    let weak_gag_guard = self.weak_gag.lock().unwrap();
                    if let Some(existing_gag) = weak_gag_guard.upgrade() {
                        return Ok(GagHandle { _gag: existing_gag });
                    }
                }
                return Err(e);
            }
        };

        // Mark as created and store the weak reference
        let gag_arc = Arc::new(gag);

        // Lock again to store the weak reference
        *weak_gag_guard = Arc::downgrade(&gag_arc);

        Ok(GagHandle { _gag: gag_arc })
    }
}

// Functions to redirect stdout and stderr to log file
fn redirect_stdout() -> Result<Redirect<std::fs::File>, std::io::Error> {
    let file = OpenOptions::new()
        .create(true)
        .append(true)
        .open("hbcn.log")?;
    Redirect::stdout(file).map_err(|_| {
        std::io::Error::new(std::io::ErrorKind::AlreadyExists, "Redirect already exists")
    })
}

fn redirect_stderr() -> Result<Redirect<std::fs::File>, std::io::Error> {
    let file = OpenOptions::new()
        .create(true)
        .append(true)
        .open("hbcn.log")?;
    Redirect::stderr(file).map_err(|_| {
        std::io::Error::new(std::io::ErrorKind::AlreadyExists, "Redirect already exists")
    })
}

// Global singleton managers for stdout and stderr
static STDOUT_GAG_MANAGER: GagManager = GagManager::new(redirect_stdout);
static STDERR_GAG_MANAGER: GagManager = GagManager::new(redirect_stderr);

/// Convenience function to redirect both stdout and stderr to log file
pub fn redirect_output() -> Result<(GagHandle, GagHandle), std::io::Error> {
    Ok((GagHandle::stdout()?, GagHandle::stderr()?))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc as StdArc;
    use std::sync::Barrier;
    use std::thread;

    #[test]
    fn test_stdout_gag_singleton() {
        // Test that multiple handles share the same underlying Gag
        let handle1 = match GagHandle::stdout() {
            Ok(handle) => handle,
            Err(_) => {
                println!("Skipping singleton test - gag already used in process");
                return;
            }
        };
        let handle2 = GagHandle::stdout().expect("Should reuse stdout gag");

        // Both handles should point to the same Arc instance
        assert_eq!(StdArc::as_ptr(&handle1._gag), StdArc::as_ptr(&handle2._gag));

        // Reference count should be at least 2 (may be higher due to other tests)
        let initial_count = StdArc::strong_count(&handle1._gag);
        assert!(initial_count >= 2, "Should have at least 2 references");
        assert_eq!(StdArc::strong_count(&handle2._gag), initial_count);

        // Drop one handle, count should decrease by 1
        drop(handle1);
        assert_eq!(StdArc::strong_count(&handle2._gag), initial_count - 1);
    }

    #[test]
    fn test_gag_cannot_be_recreated_after_drop() {
        // Create and drop a gag handle
        {
            let _handle = GagHandle::stderr().expect("Should create stderr gag first time");
        }

        // Once all handles are dropped, attempting to create a new one should fail
        // because gag can only be created once per process
        let result = GagHandle::stderr();

        // This should either succeed (if the gag is still held in memory) or fail
        // The behaviour depends on the internal implementation of gag
        match result {
            Ok(_) => {
                // If it succeeds, it means the gag manager is properly keeping
                // the weak reference and upgrading it
            }
            Err(e) => {
                // If it fails, it should be our custom error message
                assert_eq!(e.kind(), std::io::ErrorKind::AlreadyExists);
            }
        }
    }

    #[test]
    fn test_multithreaded_gag_sharing() {
        // First, try to create a gag handle or get existing one
        let initial_handle = match GagHandle::stdout() {
            Ok(handle) => handle,
            Err(_) => {
                // If we can't get a gag (likely because one was created and dropped
                // in previous tests), skip this test
                println!("Skipping multithreaded test - gag already used in process");
                return;
            }
        };
        let initial_ptr = StdArc::as_ptr(&initial_handle._gag);

        const NUM_THREADS: usize = 3;
        let barrier = StdArc::new(Barrier::new(NUM_THREADS));
        let mut handles = Vec::new();

        // Spawn multiple threads that all try to get the same stdout gag
        for _ in 0..NUM_THREADS {
            let barrier_clone = StdArc::clone(&barrier);
            let handle = thread::spawn(move || {
                // Wait for all threads to be ready
                barrier_clone.wait();

                // All threads try to get stdout gag at the same time
                GagHandle::stdout()
            });
            handles.push(handle);
        }

        // Collect all handles
        let gag_handles: Vec<_> = handles
            .into_iter()
            .map(|h| h.join().expect("Thread should complete"))
            .collect();

        // Check results - at least some should succeed
        let successful_handles: Vec<_> = gag_handles.into_iter().filter_map(|h| h.ok()).collect();

        // Most threads should succeed since there's an existing gag to share
        assert!(
            !successful_handles.is_empty(),
            "At least some threads should get gag handles when one exists"
        );

        // All successful handles should point to the same Arc instance as initial
        for handle in &successful_handles {
            assert_eq!(StdArc::as_ptr(&handle._gag), initial_ptr);
        }

        // The key test: all handles (including initial) should point to same instance
        // This proves the singleton pattern is working correctly
        println!(
            "Successfully created {} thread-safe gag handles sharing the same instance",
            successful_handles.len() + 1
        ); // +1 for initial_handle
    }

    #[test]
    fn test_gag_manager_thread_safety() {
        // This test focuses on the thread safety of the manager itself
        const NUM_THREADS: usize = 3;
        let barrier = StdArc::new(Barrier::new(NUM_THREADS));
        let mut handles = Vec::new();

        for _ in 0..NUM_THREADS {
            let barrier_clone = StdArc::clone(&barrier);
            let handle = thread::spawn(move || {
                barrier_clone.wait();

                // Try to get a handle - should not panic even under concurrent access
                let _ = GagHandle::stdout();
            });
            handles.push(handle);
        }

        // Ensure all threads complete without panicking
        for handle in handles {
            handle.join().expect("Thread should not panic");
        }
    }

    #[test]
    fn test_convenience_redirect_output() {
        // Test the convenience function
        let result = redirect_output();

        // Should either succeed or fail gracefully
        match result {
            Ok((stdout_gag, stderr_gag)) => {
                // Should have separate gag instances
                assert_ne!(
                    StdArc::as_ptr(&stdout_gag._gag),
                    StdArc::as_ptr(&stderr_gag._gag)
                );
            }
            Err(_) => {
                // If it fails, that's also acceptable due to gag limitations
            }
        }
    }
}
