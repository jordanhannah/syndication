use anyhow::{Context, Result};
use chrono::{DateTime, Duration, Utc};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::Mutex;

/// NCTS OAuth2 token endpoint
const TOKEN_ENDPOINT: &str = "https://api.healthterminologies.gov.au/oauth2/token";

/// OAuth2 token response from NCTS
#[derive(Debug, Clone, Serialize, Deserialize)]
struct TokenResponse {
    access_token: String,
    token_type: String,
    expires_in: i64,
}

/// Cached token with expiry tracking
#[derive(Debug, Clone)]
struct CachedToken {
    access_token: String,
    expires_at: DateTime<Utc>,
}

impl CachedToken {
    /// Check if token is expired or will expire within the next 60 seconds
    fn is_expired(&self) -> bool {
        let now = Utc::now();
        let buffer = Duration::seconds(60);
        self.expires_at - buffer < now
    }
}

/// OAuth2 token manager for NCTS authentication
pub struct TokenManager {
    client: Client,
    client_id: String,
    client_secret: String,
    cached_token: Arc<Mutex<Option<CachedToken>>>,
}

impl TokenManager {
    /// Create a new token manager with NCTS credentials
    pub fn new(client_id: String, client_secret: String) -> Result<Self> {
        let client = Client::builder()
            .user_agent("NCTS-Syndication/0.1.0")
            .build()
            .context("Failed to create HTTP client")?;

        Ok(Self {
            client,
            client_id,
            client_secret,
            cached_token: Arc::new(Mutex::new(None)),
        })
    }

    /// Get a valid access token, refreshing if necessary
    pub async fn get_token(&self) -> Result<String> {
        let mut cached = self.cached_token.lock().await;

        // Return cached token if still valid
        if let Some(token) = cached.as_ref() {
            if !token.is_expired() {
                return Ok(token.access_token.clone());
            }
        }

        // Request new token
        let token = self.request_token().await?;

        // Cache the new token
        let expires_at = Utc::now() + Duration::seconds(token.expires_in);
        let cached_token = CachedToken {
            access_token: token.access_token.clone(),
            expires_at,
        };
        *cached = Some(cached_token);

        Ok(token.access_token)
    }

    /// Request a new access token from NCTS
    async fn request_token(&self) -> Result<TokenResponse> {
        let params = [
            ("grant_type", "client_credentials"),
            ("client_id", &self.client_id),
            ("client_secret", &self.client_secret),
        ];

        let response = self
            .client
            .post(TOKEN_ENDPOINT)
            .form(&params)
            .send()
            .await
            .context("Failed to request access token")?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            anyhow::bail!(
                "Failed to obtain access token: HTTP {} - {}",
                status,
                error_text
            );
        }

        let token_response: TokenResponse = response
            .json()
            .await
            .context("Failed to parse token response")?;

        println!("Successfully obtained access token (expires in {} seconds)", token_response.expires_in);

        Ok(token_response)
    }

    /// Create a token manager from environment variables
    pub fn from_env() -> Result<Self> {
        let client_id = std::env::var("NCTS_CLIENT_ID")
            .context("NCTS_CLIENT_ID environment variable not set")?;
        let client_secret = std::env::var("NCTS_CLIENT_SECRET")
            .context("NCTS_CLIENT_SECRET environment variable not set")?;

        Self::new(client_id, client_secret)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_token_expiry() {
        let token = CachedToken {
            access_token: "test".to_string(),
            expires_at: Utc::now() - Duration::seconds(1),
        };
        assert!(token.is_expired());

        let token = CachedToken {
            access_token: "test".to_string(),
            expires_at: Utc::now() + Duration::seconds(120),
        };
        assert!(!token.is_expired());

        // Should be considered expired within 60s buffer
        let token = CachedToken {
            access_token: "test".to_string(),
            expires_at: Utc::now() + Duration::seconds(30),
        };
        assert!(token.is_expired());
    }
}
