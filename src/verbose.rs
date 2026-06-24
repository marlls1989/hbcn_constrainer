//! Global verbose flag.
//!
//! When verbose mode is enabled (the top-level `--verbose`/`-v` flag), the `analyse` and
//! `constrain` commands print extra progress messages to stderr. The LP solver backends live
//! in the external `lp_solver` crate and write their own progress to stdout; hbcn no longer
//! redirects or suppresses that output.

use std::sync::atomic::{AtomicBool, Ordering};

/// Global verbose flag — set to true to enable verbose output.
static VERBOSE: AtomicBool = AtomicBool::new(false);

/// Set the global verbose flag.
pub fn set_verbose(verbose: bool) {
    VERBOSE.store(verbose, Ordering::Release);
}

/// Get the current verbose flag value.
pub fn is_verbose() -> bool {
    VERBOSE.load(Ordering::Acquire)
}
