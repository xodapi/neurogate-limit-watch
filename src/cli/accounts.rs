use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use serde::Deserialize;

use super::config::dirs_or_default;

#[derive(Debug, Clone, Deserialize)]
pub struct AccountConfig {
    pub api_key_env: Option<String>,
    pub api_base: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AccountsConfig {
    pub accounts: HashMap<String, AccountConfig>,
}

impl AccountsConfig {
    pub fn load() -> Result<Self, String> {
        let Some(path) = default_accounts_path() else {
            return Ok(AccountsConfig {
                accounts: HashMap::new(),
            });
        };
        if !path.is_file() {
            return Ok(AccountsConfig {
                accounts: HashMap::new(),
            });
        }
        let raw = fs::read_to_string(&path)
            .map_err(|e| format!("cannot read accounts {}: {e}", path.display()))?;
        toml::from_str(&raw).map_err(|e| format!("invalid accounts {}: {e}", path.display()))
    }

    pub fn resolve(&self, name: &str) -> Result<AccountConfig, String> {
        self.accounts
            .get(name)
            .cloned()
            .ok_or_else(|| format!("account '{name}' not found in accounts file"))
    }

    pub fn list_names(&self) -> Vec<String> {
        let mut names: Vec<String> = self.accounts.keys().cloned().collect();
        names.sort();
        names
    }
}

fn default_accounts_path() -> Option<PathBuf> {
    let home = dirs_or_default()?;
    let config_dir = if cfg!(windows) {
        home.join("nglimit")
    } else {
        home.join(".config").join("nglimit")
    };
    let path = config_dir.join("accounts.toml");
    if path.is_file() {
        Some(path)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_accounts_when_no_file() {
        let accounts = AccountsConfig::load().unwrap();
        assert!(accounts.accounts.is_empty());
    }

    #[test]
    fn resolve_returns_error_for_unknown() {
        let accounts = AccountsConfig {
            accounts: HashMap::new(),
        };
        assert!(accounts.resolve("nonexistent").is_err());
    }

    #[test]
    fn list_names_sorted() {
        let mut accounts = HashMap::new();
        accounts.insert(
            "prod".to_string(),
            AccountConfig {
                api_key_env: Some("PROD_KEY".to_string()),
                api_base: None,
            },
        );
        accounts.insert(
            "dev".to_string(),
            AccountConfig {
                api_key_env: Some("DEV_KEY".to_string()),
                api_base: Some("https://dev.example.com".to_string()),
            },
        );
        let config = AccountsConfig { accounts };
        assert_eq!(config.list_names(), vec!["dev", "prod"]);
    }

    #[test]
    fn resolve_returns_account() {
        let mut accounts = HashMap::new();
        accounts.insert(
            "main".to_string(),
            AccountConfig {
                api_key_env: None,
                api_base: Some("https://api.example.com".to_string()),
            },
        );
        let config = AccountsConfig { accounts };
        let acct = config.resolve("main").unwrap();
        assert_eq!(acct.api_base.unwrap(), "https://api.example.com");
        assert!(acct.api_key_env.is_none());
    }
}
