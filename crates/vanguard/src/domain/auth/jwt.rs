//! JWT token handling.

use chrono::{Duration, Utc};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, TokenData, Validation};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::ApiError;

/// JWT claims for access tokens
#[derive(Debug, Serialize, Deserialize)]
pub struct AccessTokenClaims {
    /// Subject (user ID)
    pub sub: Uuid,
    /// Username
    pub username: String,
    /// User role
    pub role: String,
    /// Expiration time (Unix timestamp)
    pub exp: i64,
    /// Issued at (Unix timestamp)
    pub iat: i64,
    /// Token type
    pub token_type: String,
}

/// JWT claims for refresh tokens
#[derive(Debug, Serialize, Deserialize)]
pub struct RefreshTokenClaims {
    /// Subject (user ID)
    pub sub: Uuid,
    /// Expiration time (Unix timestamp)
    pub exp: i64,
    /// Issued at (Unix timestamp)
    pub iat: i64,
    /// Token type
    pub token_type: String,
    /// Session ID for revocation
    pub session_id: Uuid,
}

/// JWT token manager
pub struct JwtManager {
    encoding_key: EncodingKey,
    decoding_key: DecodingKey,
    access_expiration: i64,
    refresh_expiration: i64,
}

impl JwtManager {
    /// Create a new JWT manager
    pub fn new(secret: &str, access_expiration: i64, refresh_expiration: i64) -> Self {
        Self {
            encoding_key: EncodingKey::from_secret(secret.as_bytes()),
            decoding_key: DecodingKey::from_secret(secret.as_bytes()),
            access_expiration,
            refresh_expiration,
        }
    }

    /// Generate an access token
    pub fn generate_access_token(
        &self,
        user_id: Uuid,
        username: &str,
        role: &str,
    ) -> Result<String, ApiError> {
        let now = Utc::now();
        let exp = now + Duration::seconds(self.access_expiration);

        let claims = AccessTokenClaims {
            sub: user_id,
            username: username.to_string(),
            role: role.to_string(),
            exp: exp.timestamp(),
            iat: now.timestamp(),
            token_type: "access".to_string(),
        };

        encode(&Header::default(), &claims, &self.encoding_key)
            .map_err(|e| ApiError::Token(e.to_string()))
    }

    /// Generate a refresh token
    pub fn generate_refresh_token(
        &self,
        user_id: Uuid,
        session_id: Uuid,
    ) -> Result<String, ApiError> {
        let now = Utc::now();
        let exp = now + Duration::seconds(self.refresh_expiration);

        let claims = RefreshTokenClaims {
            sub: user_id,
            exp: exp.timestamp(),
            iat: now.timestamp(),
            token_type: "refresh".to_string(),
            session_id,
        };

        encode(&Header::default(), &claims, &self.encoding_key)
            .map_err(|e| ApiError::Token(e.to_string()))
    }

    /// Verify and decode an access token
    pub fn verify_access_token(&self, token: &str) -> Result<AccessTokenClaims, ApiError> {
        let token_data: TokenData<AccessTokenClaims> =
            decode(token, &self.decoding_key, &Validation::default())
                .map_err(|e| ApiError::Token(e.to_string()))?;

        if token_data.claims.token_type != "access" {
            return Err(ApiError::Token("Invalid token type".to_string()));
        }

        Ok(token_data.claims)
    }

    /// Verify and decode a refresh token
    pub fn verify_refresh_token(&self, token: &str) -> Result<RefreshTokenClaims, ApiError> {
        let token_data: TokenData<RefreshTokenClaims> =
            decode(token, &self.decoding_key, &Validation::default())
                .map_err(|e| ApiError::Token(e.to_string()))?;

        if token_data.claims.token_type != "refresh" {
            return Err(ApiError::Token("Invalid token type".to_string()));
        }

        Ok(token_data.claims)
    }

    /// Get access token expiration in seconds
    pub fn access_expiration(&self) -> i64 {
        self.access_expiration
    }

    /// Get refresh token expiration in seconds
    pub fn refresh_expiration(&self) -> i64 {
        self.refresh_expiration
    }
}
