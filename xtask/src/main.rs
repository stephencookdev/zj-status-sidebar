use std::env;
use std::path::Path;
use std::process::{Command, ExitCode};

fn main() -> ExitCode {
    let args: Vec<String> = env::args().collect();
    
    if args.len() < 2 {
        eprintln!("Usage: cargo xtask <command>");
        eprintln!("Commands:");
        eprintln!("  install    Build and install the plugin to ~/.config/zellij/plugins/");
        return ExitCode::FAILURE;
    }

    match args[1].as_str() {
        "install" => install(),
        cmd => {
            eprintln!("Unknown command: {}", cmd);
            eprintln!("Run 'cargo xtask' for available commands");
            ExitCode::FAILURE
        }
    }
}

fn install() -> ExitCode {
    println!("Building zj-status-sidebar in release mode...");
    
    // Build the project in release mode
    let build_status = Command::new("cargo")
        .args(&["build", "--release"])
        .status();

    match build_status {
        Ok(status) if status.success() => {
            println!("Build successful!");
        }
        _ => {
            eprintln!("Build failed!");
            return ExitCode::FAILURE;
        }
    }

    // Create the plugins directory if it doesn't exist
    let home = env::var("HOME").unwrap_or_else(|_| {
        eprintln!("Could not determine HOME directory");
        std::process::exit(1);
    });
    
    let plugin_dir = format!("{}/.config/zellij/plugins", home);
    std::fs::create_dir_all(&plugin_dir).unwrap_or_else(|e| {
        eprintln!("Failed to create plugin directory: {}", e);
        std::process::exit(1);
    });

    // Copy the wasm file
    let source = "target/wasm32-wasip1/release/zj-status-sidebar.wasm";
    let destination = format!("{}/zj-status-sidebar.wasm", plugin_dir);
    
    println!("Installing plugin to {}...", destination);
    
    match std::fs::copy(source, &destination) {
        Ok(_) => {
            println!("Plugin installed successfully!");
            println!("You can now use it in your Zellij configuration");
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("Failed to install plugin: {}", e);
            if !Path::new(source).exists() {
                eprintln!("The compiled plugin was not found at {}", source);
                eprintln!("Make sure the build completed successfully");
            }
            ExitCode::FAILURE
        }
    }
}