//! Example demonstrating the thread-safe singleton redirection pattern
//!
//! This example shows how to use the output redirection utilities to redirect
//! verbose output from LP solvers to a log file across multiple threads safely.

use hbcn::output_suppression::{GagHandle, redirect_output};
use std::thread;
use std::time::Duration;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Output Redirection Demo ===\n");

    // Example 1: Basic usage
    println!("1. Basic stdout redirection to lp_solver.log:");
    {
        let _gag = GagHandle::stdout()?;
        println!("This line will be redirected to lp_solver.log!");
        eprintln!("But this will still appear on stderr");
    }
    println!("Output restored after gag is dropped\n");

    // Example 2: Multiple handles sharing the same gag
    println!("2. Multiple handles sharing the same gag:");
    {
        let gag1 = GagHandle::stdout()?;
        let gag2 = GagHandle::stdout()?; // Reuses the same underlying Gag

        println!("This will be redirected to lp_solver.log");

        drop(gag1);
        println!("Still redirected because gag2 is active");

        drop(gag2);
    }
    println!("Output restored when all handles are dropped\n");

    // Example 3: Redirect both stdout and stderr
    println!("3. Redirecting both stdout and stderr to lp_solver.log:");
    {
        let (_stdout_gag, _stderr_gag) = redirect_output()?;
        println!("This stdout message is redirected to log file");
        eprintln!("This stderr message is also redirected to log file");
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

                    // This would normally print, but output is redirected to log file
                    println!("Thread {} output (redirected to lp_solver.log)", i);

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
