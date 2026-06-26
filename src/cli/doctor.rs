use std::path::PathBuf;

use vimit as ng;

use super::accounts::AccountsConfig;
use super::config::{dirs_or_default, Config};

pub fn run_doctor() -> Result<i32, String> {
    let mut ok = true;

    println!("vimit doctor — system diagnostics");
    println!();

    // config.toml
    let config_path = default_config_file_path();
    match &config_path {
        Some(p) if p.is_file() => {
            println!("  [✓] config.toml found: {}", p.display());
            match Config::load(config_path.as_ref()) {
                Ok(_) => println!("  [✓] config.toml is valid TOML"),
                Err(e) => {
                    println!("  [✗] config.toml parse error: {e}");
                    ok = false;
                }
            }
        }
        Some(p) => {
            println!("  [ ] config.toml not found: {}", p.display());
            println!("       hint: create with: vimit init");
        }
        None => {
            println!("  [ ] config.toml: no config directory found");
            println!("       hint: create with: vimit init");
        }
    }

    // accounts.toml
    match AccountsConfig::load() {
        Ok(accts) => {
            let names = accts.list_names();
            if names.is_empty() {
                println!("  [ ] accounts.toml: no accounts configured");
            } else {
                println!(
                    "  [✓] accounts.toml: {} account(s): {}",
                    names.len(),
                    names.join(", ")
                );
            }
        }
        Err(e) => {
            println!("  [✗] accounts.toml error: {e}");
            ok = false;
        }
    }

    // Environment
    println!();
    println!("  environment:");
    let home = std::env::var("HOME").or_else(|_| std::env::var("USERPROFILE"));
    match &home {
        Ok(h) => println!("       HOME: {h}"),
        Err(_) => println!("       HOME: (not set)"),
    }
    let api_base = std::env::var("VIBEMODE_API_BASE")
        .or_else(|_| std::env::var("VIBEMOD_API_BASE"))
        .or_else(|_| std::env::var("NEUROGATE_API_BASE"));
    match &api_base {
        Ok(v) => {
            let var_name = if std::env::var("VIBEMODE_API_BASE").is_ok() {
                "VIBEMODE_API_BASE"
            } else if std::env::var("VIBEMOD_API_BASE").is_ok() {
                "VIBEMOD_API_BASE"
            } else {
                "NEUROGATE_API_BASE"
            };
            println!("       {var_name}: {v}");
        }
        Err(_) => println!("       VIBEMODE_API_BASE: (not set, will use default)"),
    }
    let api_key = std::env::var("VIBEMODE_API_KEY")
        .or_else(|_| std::env::var("VIBEMOD_API_KEY"))
        .or_else(|_| std::env::var("NEUROGATE_API_KEY"));
    match &api_key {
        Ok(_) => {
            let var_name = if std::env::var("VIBEMODE_API_KEY").is_ok() {
                "VIBEMODE_API_KEY"
            } else if std::env::var("VIBEMOD_API_KEY").is_ok() {
                "VIBEMOD_API_KEY"
            } else {
                "NEUROGATE_API_KEY"
            };
            println!("       {var_name}: (set)");
        }
        Err(_) => println!("       VIBEMODE_API_KEY: (not set, demo data only)"),
    }

    // API connectivity
    println!();
    let api_key_val = api_key.unwrap_or_default();
    let api_base_val = api_base.unwrap_or_else(|_| ng::DEFAULT_API_BASE.to_string());
    if !api_key_val.is_empty() {
        print!("  testing API connection to {api_base_val}... ");
        let http = ng::HttpClient::new(ng::USER_AGENT)?;
        match http.fetch_me(&api_key_val, &api_base_val) {
            Ok(payload) => {
                let windows = ng::summarize_me(&payload, 75.0, 90.0);
                println!("OK ({} window(s))", windows.len());
            }
            Err(e) => {
                println!("FAILED");
                println!("       error: {e}");
                println!("       hint: check VIBEMODE_API_KEY and network");
                ok = false;
            }
        }
    } else {
        println!("  API connectivity: skipped (no API key)");
        println!("       hint: set VIBEMODE_API_KEY or use --demo");
    }

    // Summary
    println!();
    if ok {
        println!("  status: all checks passed");
        Ok(0)
    } else {
        println!("  status: some checks failed (see above)");
        Ok(1)
    }
}

fn default_config_file_path() -> Option<PathBuf> {
    let home = dirs_or_default()?;
    let config_dir = if cfg!(windows) {
        home.join("vimit")
    } else {
        home.join(".config").join("vimit")
    };
    Some(config_dir.join("config.toml"))
}
