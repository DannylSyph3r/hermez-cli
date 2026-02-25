use crate::auth::config::{API_URL, Config, UserInfo, delete_config, load_config, save_config};
use crate::error::HermezError;
use colored::Colorize;
use serde::Deserialize;
use std::io::Write;

const VALIDATE_ENDPOINT: &str = "/api/v1/api-keys/validate";

// Wrapper matching the backend ApiResponse<T> envelope
#[derive(Deserialize)]
struct ApiResponse<T> {
    data: T,
}

// Matches ApiKeyValidationResponse from the backend service
#[derive(Deserialize)]
struct ValidateData {
    #[serde(rename = "userId")]
    user_id: String,
    email: String,
    tier: String,
    valid: bool,
}

enum KeyStatus {
    /// Server confirmed the key is still valid.
    Valid,
    /// Server returned 401 — key has been revoked or deleted.
    Revoked,
    /// Network unreachable — cannot verify, fail open.
    Unreachable,
}

/// Silently validates a stored API key without any user-visible output.
/// Used on login entry to detect stale/revoked keys before showing the prompt.
async fn validate_stored_key(api_key: &str) -> KeyStatus {
    let url = format!("{}{}", API_URL, VALIDATE_ENDPOINT);
    let client = reqwest::Client::new();

    let result = client
        .get(&url)
        .header("Authorization", format!("Bearer {}", api_key))
        .send()
        .await;

    match result {
        Ok(response) => {
            if response.status() == reqwest::StatusCode::UNAUTHORIZED {
                KeyStatus::Revoked
            } else if response.status().is_success() {
                // Also check the valid field in the body — handles soft-revoked keys
                match response.json::<ApiResponse<ValidateData>>().await {
                    Ok(body) if body.data.valid => KeyStatus::Valid,
                    _ => KeyStatus::Revoked,
                }
            } else {
                // Any other server error — fail open, don't block the user
                KeyStatus::Unreachable
            }
        }
        // Network error — fail open
        Err(_) => KeyStatus::Unreachable,
    }
}

pub async fn login() -> Result<(), HermezError> {
    if let Some(existing) = load_config()? {
        match validate_stored_key(&existing.api_key).await {
            KeyStatus::Revoked => {
                // Key is dead — wipe it and go straight to re-auth
                delete_config()?;
                println!(
                    "{} Your stored API key is no longer valid. Please log in again.",
                    "!".yellow().bold()
                );
            }
            KeyStatus::Valid | KeyStatus::Unreachable => {
                // Key is good or we can't verify — show the normal already-logged-in prompt
                print!(
                    "Already logged in as {}. Log in as a different account? [y/N]: ",
                    existing.user.email.bold()
                );
                std::io::stdout().flush().ok();
                let mut response = String::new();
                std::io::stdin().read_line(&mut response).ok();
                if !response.trim().eq_ignore_ascii_case("y") {
                    return Ok(());
                }
            }
        }
    }

    let api_key = rpassword::prompt_password("Enter your API key: ")
        .map_err(|e| HermezError::Config(format!("Failed to read API key: {}", e)))?;

    let api_key = api_key.trim().to_string();

    if api_key.is_empty() {
        return Err(HermezError::AuthFailed(
            "API key cannot be empty".to_string(),
        ));
    }

    if !api_key.starts_with("hk_") {
        return Err(HermezError::AuthFailed(
            "Invalid API key format. Keys must start with hk_".to_string(),
        ));
    }

    let url = format!("{}{}", API_URL, VALIDATE_ENDPOINT);

    let client = reqwest::Client::new();
    let response = client
        .get(&url)
        .header("Authorization", format!("Bearer {}", api_key))
        .send()
        .await
        .map_err(|e| HermezError::ConnectionFailed(e.to_string()))?;

    if response.status() == reqwest::StatusCode::UNAUTHORIZED {
        return Err(HermezError::AuthFailed(
            "Invalid API key. Please check your key and try again.".to_string(),
        ));
    }

    if !response.status().is_success() {
        return Err(HermezError::AuthFailed(format!(
            "Validation failed with status: {}",
            response.status()
        )));
    }

    let body: ApiResponse<ValidateData> = response
        .json()
        .await
        .map_err(|e| HermezError::AuthFailed(format!("Failed to parse response: {}", e)))?;

    if !body.data.valid {
        return Err(HermezError::AuthFailed(
            "API key is invalid or has been revoked.".to_string(),
        ));
    }

    let config = Config {
        api_key,
        user: UserInfo {
            id: body.data.user_id,
            email: body.data.email.clone(),
            tier: body.data.tier,
        },
    };

    save_config(&config)?;

    println!(
        "{} Logged in as {}",
        "✓".green().bold(),
        body.data.email.bold()
    );
    println!("  API key stored in {}", "~/.hermez/config.json".dimmed());

    Ok(())
}

pub fn logout() -> Result<(), HermezError> {
    delete_config()?;
    println!("{} Logged out successfully", "✓".green().bold());
    Ok(())
}

/// Returns the API key for authenticated operations.
/// Checks HERMEZ_API_KEY env var first, falls back to config file.
pub fn require_auth() -> Result<String, HermezError> {
    // Env var takes priority — useful for CI/CD environments
    if let Ok(key) = std::env::var("HERMEZ_API_KEY") {
        if !key.is_empty() {
            return Ok(key);
        }
    }

    // Fall back to config file
    match load_config()? {
        Some(config) => Ok(config.api_key),
        None => Err(HermezError::NotAuthenticated),
    }
}
