use crate::auth::config::{
    Config, ServerConfig, UserInfo, delete_config, load_config, save_config,
};
use crate::error::HermezError;
use colored::Colorize;
use serde::Deserialize;

const VALIDATE_ENDPOINT: &str = "/api/v1/api-keys/validate";

// Wrapper matching the backend ApiResponse<T> envelope
#[derive(Deserialize)]
struct ApiResponse<T> {
    data: T,
}

// Matches ApiKeyValidationResponse after our backend fix
#[derive(Deserialize)]
struct ValidateData {
    #[serde(rename = "userId")]
    user_id: String,
    email: String,
    tier: String,
    valid: bool,
}

pub async fn login() -> Result<(), HermezError> {
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

    // Load default server config or existing one
    let server = load_config()?.map(|c| c.server).unwrap_or_default();

    let url = format!("{}{}", server.api_url, VALIDATE_ENDPOINT);

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
        server,
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
