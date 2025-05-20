use gamecode_tools::logging;
use log::{debug, error, info, trace, warn, LevelFilter};

fn main() {
    // Initialize simple logging with env_logger
    env_logger::Builder::from_default_env()
        .filter_level(LevelFilter::Debug)
        .init();

    // Log messages at different levels
    trace!("This is a trace message");
    debug!("This is a debug message");
    info!("This is an info message");
    warn!("This is a warning message");
    error!("This is an error message");

    // Run some tools to demonstrate logging
    println!("Try running the tools with RUST_LOG=debug to see debug output");
    println!("Example: RUST_LOG=debug cargo run --example directory_list");
}