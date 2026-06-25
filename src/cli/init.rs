use std::io::{self, Write};

use super::config::dirs_or_default;

pub fn run_init() -> Result<i32, String> {
    let home = dirs_or_default().ok_or_else(|| "cannot determine home directory".to_string())?;
    let config_dir = if cfg!(windows) {
        home.join("nglimit")
    } else {
        home.join(".config").join("nglimit")
    };

    println!("nglimit init — interactive setup");
    println!();

    // Create config directory
    if config_dir.is_dir() {
        println!("  config directory: {} (exists)", config_dir.display());
    } else {
        print!("  creating config directory: {}... ", config_dir.display());
        io::stdout().flush().unwrap();
        std::fs::create_dir_all(&config_dir)
            .map_err(|e| format!("cannot create config dir: {e}"))?;
        println!("done");
    }

    // Create config.toml
    let config_file = config_dir.join("config.toml");
    if config_file.is_file() {
        println!(
            "  config.toml: {} (exists, skipping)",
            config_file.display()
        );
    } else {
        let default_config = r#"# nglimit configuration
# See https://github.com/xodapi/neurogate-limit-watch for docs

# Default thresholds
# warning = 75
# danger = 90

# Color theme (btop, dracula, catppuccin, tokyo-night, gruvbox, nord, ...)
# theme = "btop"

# Monitor preset (full, compact, mini)
# preset = "full"

# Notify on threshold breach
# notify = false

# Poll interval in seconds (0 = single run)
# watch = 0
"#;
        print!("  creating config.toml... ");
        io::stdout().flush().unwrap();
        std::fs::write(&config_file, default_config)
            .map_err(|e| format!("cannot write config.toml: {e}"))?;
        println!("done");
    }

    // Optionally create .env
    let env_file = config_dir.join(".env");
    if env_file.is_file() {
        println!("  .env: {} (exists, skipping)", env_file.display());
    } else {
        print!("\n  ? create .env with NEUROGATE_API_KEY? [y/N] ");
        io::stdout().flush().unwrap();
        let mut answer = String::new();
        io::stdin().read_line(&mut answer).unwrap();
        let answer = answer.trim().to_lowercase();
        if answer == "y" || answer == "yes" {
            print!("  ? enter your API key: ");
            io::stdout().flush().unwrap();
            let mut key = String::new();
            io::stdin().read_line(&mut key).unwrap();
            let key = key.trim().to_string();
            if !key.is_empty() {
                let env_content = format!(
                    "NEUROGATE_API_KEY={key}\n# NEUROGATE_API_BASE=https://api.neurogate.space\n"
                );
                std::fs::write(&env_file, env_content)
                    .map_err(|e| format!("cannot write .env: {e}"))?;
                println!("  .env created with NEUROGATE_API_KEY");
                println!("  hint: you can also set the env var directly or use --api-key-env");
            } else {
                println!("  skipping — empty key");
            }
        }
    }

    // Optionally test connection
    print!("\n  ? test API connection now? [Y/n] ");
    io::stdout().flush().unwrap();
    let mut answer = String::new();
    io::stdin().read_line(&mut answer).unwrap();
    let answer = answer.trim().to_lowercase();
    if answer != "n" && answer != "no" {
        test_connection()?;
    }

    println!();
    println!("  setup complete!");
    println!("  run 'nglimit --monitor' to start the dashboard");
    println!("  or 'nglimit --doctor' for diagnostics");
    Ok(0)
}

fn test_connection() -> Result<(), String> {
    let dotenv = neurogate_limit_watch::load_dotenv_custom(None).unwrap_or_default();
    let api_key =
        neurogate_limit_watch::config_value("NEUROGATE_API_KEY", &dotenv).unwrap_or_default();
    if api_key.is_empty() {
        println!("  skipping connection test: no API key found");
        return Ok(());
    }
    let api_base = neurogate_limit_watch::config_value("NEUROGATE_API_BASE", &dotenv)
        .unwrap_or_else(|| neurogate_limit_watch::DEFAULT_API_BASE.to_string());
    print!("  testing {api_base}... ");
    io::stdout().flush().unwrap();
    let http = neurogate_limit_watch::HttpClient::new(neurogate_limit_watch::USER_AGENT)?;
    match http.fetch_me(&api_key, &api_base) {
        Ok(payload) => {
            let windows = neurogate_limit_watch::summarize_me(&payload, 75.0, 90.0);
            println!("OK ({} window(s))", windows.len());
            Ok(())
        }
        Err(e) => {
            println!("FAILED");
            println!("  error: {e}");
            println!("  hint: check your API key and network");
            Ok(())
        }
    }
}
