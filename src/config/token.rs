use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::time::SystemTime;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Token {
    pub token_type: Option<String>,
    pub access_token: Option<String>,
    pub refresh_token: Option<String>,
    pub expiry_time: f64,
    pub session_id: Option<String>,
    pub country_code: Option<String>,
    pub user_id: Option<u64>,
    #[serde(default)]
    pub is_pkce: bool,
}

impl Token {
    /// Returns the full path to the token file: `~/.tdl/token.json`
    pub fn config_path() -> PathBuf {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("~"))
            .join(".tdl")
            .join("token.json")
    }

    /// Load token from the JSON file.
    ///
    /// Returns a default (empty) token if the file does not exist.
    pub fn load() -> Result<Self> {
        let path = Self::config_path();

        if !path.exists() {
            return Ok(Self::default());
        }

        let contents = fs::read_to_string(&path)
            .with_context(|| format!("Failed to read token from {}", path.display()))?;

        let token: Token = serde_json::from_str(&contents)
            .with_context(|| format!("Failed to parse token from {}", path.display()))?;

        Ok(token)
    }

    /// Save token to the JSON file.
    ///
    /// Creates the configuration directory if it does not exist.
    pub fn save(&self) -> Result<()> {
        let dir = Self::config_path()
            .parent()
            .context("Token config path has no parent directory")?
            .to_path_buf();

        if !dir.exists() {
            fs::create_dir_all(&dir)
                .with_context(|| format!("Failed to create config directory {}", dir.display()))?;
        }

        let path = Self::config_path();
        let json =
            serde_json::to_string_pretty(self).context("Failed to serialize token to JSON")?;

        fs::write(&path, &json)
            .with_context(|| format!("Failed to write token to {}", path.display()))?;

        Ok(())
    }

    /// Delete the token file from disk.
    pub fn delete() -> Result<()> {
        let path = Self::config_path();

        if path.exists() {
            fs::remove_file(&path)
                .with_context(|| format!("Failed to delete token file {}", path.display()))?;
        }

        Ok(())
    }

    /// Check whether the token is valid.
    ///
    /// A token is considered valid if it has an access token and the expiry
    /// time has not yet passed.
    pub fn is_valid(&self) -> bool {
        if self.access_token.as_ref().is_none_or(|t| t.is_empty()) {
            return false;
        }

        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs_f64();

        self.expiry_time > now
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_token_is_invalid() {
        let token = Token::default();
        assert!(!token.is_valid());
    }

    #[test]
    fn token_with_access_but_expired_is_invalid() {
        let token = Token {
            access_token: Some("test_token".to_string()),
            expiry_time: 0.0,
            ..Default::default()
        };
        assert!(!token.is_valid());
    }

    #[test]
    fn token_with_future_expiry_is_valid() {
        let far_future = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs_f64()
            + 3600.0;

        let token = Token {
            access_token: Some("test_token".to_string()),
            expiry_time: far_future,
            ..Default::default()
        };
        assert!(token.is_valid());
    }

    #[test]
    fn token_with_empty_access_token_is_invalid() {
        let far_future = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs_f64()
            + 3600.0;

        let token = Token {
            access_token: Some(String::new()),
            expiry_time: far_future,
            ..Default::default()
        };
        assert!(!token.is_valid());
    }

    #[test]
    fn roundtrip_serialization() {
        let token = Token {
            token_type: Some("Bearer".to_string()),
            access_token: Some("abc123".to_string()),
            refresh_token: Some("refresh_xyz".to_string()),
            expiry_time: 1700000000.0,
            session_id: Some("sess_001".to_string()),
            country_code: Some("US".to_string()),
            user_id: Some(12345),
            is_pkce: false,
        };

        let json = serde_json::to_string_pretty(&token).unwrap();
        let restored: Token = serde_json::from_str(&json).unwrap();

        assert_eq!(token.token_type, restored.token_type);
        assert_eq!(token.access_token, restored.access_token);
        assert_eq!(token.refresh_token, restored.refresh_token);
        assert_eq!(token.expiry_time, restored.expiry_time);
        assert_eq!(token.session_id, restored.session_id);
        assert_eq!(token.country_code, restored.country_code);
        assert_eq!(token.user_id, restored.user_id);
    }
}
