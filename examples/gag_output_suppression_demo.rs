//! Example demonstrating the thread-safe singleton Gag pattern
//!
//! This example shows how to use the output suppression utilities to suppress
//! verbose output from LP solvers across multiple threads safely.

use hbcn::lp_solver::output_suppression::{GagHandle, suppress_output};
use std::thread;
use std::time::Duration;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Output Suppression Demo ===\n");

    // Example 1: Basic usage
    println!("1. Basic stdout suppression:");
    {
        let _gag = GagHandle::stdout()?;
        println!("This line will NOT be printed to stdout!");
        eprintln!("But this will still appear on stderr");
    }
    println!("Output restored after gag is dropped\n");

    // Example 2: Multiple handles sharing the same gag
    println!("2. Multiple handles sharing the same gag:");
    {
        let gag1 = GagHandle::stdout()?;
        let gag2 = GagHandle::stdout()?; // Reuses the same underlying Gag

        println!("This won't be visible");

        drop(gag1);
        println!("Still suppressed because gag2 is active");

        drop(gag2);
    }
    println!("Output restored when all handles are dropped\n");

    // Example 3: Suppress both stdout and stderr
    println!("3. Suppressing both stdout and stderr:");
    {
        let (_stdout_gag, _stderr_gag) = suppress_output()?;
        println!("This stdout message is suppressed");
        eprintln!("This stderr message is also suppressed");
    }
    println!("Both outputs restored\n");

    // Example 4: Thread-safe usage
    println!("4. Thread-safe usage:");
    {
        let gag = GagHandle::stdout()?;

        let handles: Vec<_> = (0..3)
            .map(|i| {
                thread::spawn(move || {
                    // Each thread can safely get its own handle to the same gag
                    let _thread_gag = GagHandle::stdout().expect("Should get shared gag");

                    thread::sleep(Duration::from_millis(10));

                    // This would normally print, but output is suppressed
                    println!("Thread {} output (suppressed)", i);

                    format!("Thread {} completed", i)
                })
            })
            .collect();

        // Wait for all threads
        for handle in handles {
            let result = handle.join().unwrap();
            eprintln!("{} (printed to stderr)", result);
        }

        drop(gag);
    }
    println!("All threads completed, stdout restored\n");

    println!("=== Demo Complete ===");

    Ok(())
}
