//! Authentication service

use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use chrono::{Duration, Utc};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use redis::aio::ConnectionManager;
use redis::AsyncCommands;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    config::Config,
    constants::roles,
    db::repositories::UserRepository,
    error::{AppError, AppResult},
    models::User,
};

/// JWT claims structure
#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String, // user_id
    pub username: String,
    pub role: String,
    pub exp: i64,
    pub iat: i64,
}

/// Authentication service
pub struct AuthService;

impl AuthService {
    /// Register a new user
    pub async fn register(
        pool: &PgPool,
        username: &str,
        email: &str,
        password: &str,
        display_name: Option<&str>,
    ) -> AppResult<User> {
        // Check if username exists
        if UserRepository::find_by_username(pool, username).await?.is_some() {
            return Err(AppError::AlreadyExists("Username already taken".to_string()));
        }

        // Check if email exists
        if UserRepository::find_by_email(pool, email).await?.is_some() {
            return Err(AppError::AlreadyExists("Email already registered".to_string()));
        }

        // Hash password
        let password_hash = Self::hash_password(password)?;

        // Create user
        let user = UserRepository::create(
            pool,
            username,
            email,
            &password_hash,
            display_name,
            roles::PARTICIPANT,
        )
        .await?;

        Ok(user)
    }

    /// Login with username/email and password
    pub async fn login(
        pool: &PgPool,
        mut redis: ConnectionManager,
        config: &Config,
        identifier: &str,
        password: &str,
    ) -> AppResult<(User, String, String, i64)> {
        // Find user
        let user = UserRepository::find_by_identifier(pool, identifier)
            .await?
            .ok_or(AppError::InvalidCredentials)?;

        // Check if banned
        if user.is_currently_banned() {
            return Err(AppError::Forbidden(format!(
                "Account banned: {}",
                user.ban_reason.as_deref().unwrap_or("No reason provided")
            )));
        }

        // Verify password
        if !Self::verify_password(password, &user.password_hash)? {
            return Err(AppError::InvalidCredentials);
        }

        // Update last login
        UserRepository::update_last_login(pool, &user.id).await?;

        // Generate tokens
        let (access_token, expires_in) = Self::generate_access_token(&user, config)?;
        let refresh_token = Self::generate_refresh_token();

        // Store refresh token in Redis
        let key = format!("refresh_token:{}:{}", user.id, refresh_token);
        let expiry = config.jwt.refresh_token_expiry_days * 24 * 60 * 60;
        redis.set_ex::<_, _, ()>(&key, "1", expiry as u64).await?;

        Ok((user, access_token, refresh_token, expires_in))
    }

    /// Refresh access token
    pub async fn refresh_token(
        pool: &PgPool,
        mut redis: ConnectionManager,
        config: &Config,
        refresh_token: &str,
    ) -> AppResult<(String, String, i64)> {
        // Find the refresh token in Redis (check all users)
        let pattern = format!("refresh_token:*:{}", refresh_token);
        let keys: Vec<String> = redis::cmd("KEYS")
            .arg(&pattern)
            .query_async(&mut redis)
            .await?;

        if keys.is_empty() {
            return Err(AppError::InvalidToken);
        }

        // Extract user_id from key
        let key = &keys[0];
        let parts: Vec<&str> = key.split(':').collect();
        if parts.len() != 3 {
            return Err(AppError::InvalidToken);
        }

        let user_id = Uuid::parse_str(parts[1]).map_err(|_| AppError::InvalidToken)?;

        // Get user
        let user = UserRepository::find_by_id(pool, &user_id)
            .await?
            .ok_or(AppError::InvalidToken)?;

        // Delete old refresh token
        redis.del::<_, ()>(key).await?;

        // Generate new tokens
        let (access_token, expires_in) = Self::generate_access_token(&user, config)?;
        let new_refresh_token = Self::generate_refresh_token();

        // Store new refresh token
        let new_key = format!("refresh_token:{}:{}", user.id, new_refresh_token);
        let expiry = config.jwt.refresh_token_expiry_days * 24 * 60 * 60;
        redis.set_ex::<_, _, ()>(&new_key, "1", expiry as u64).await?;

        Ok((access_token, new_refresh_token, expires_in))
    }

    /// Logout (invalidate tokens)
    pub async fn logout(
        mut redis: ConnectionManager,
        user_id: &Uuid,
        all_sessions: bool,
    ) -> AppResult<()> {
        if all_sessions {
            // Delete all refresh tokens for user
            let pattern = format!("refresh_token:{}:*", user_id);
            let keys: Vec<String> = redis::cmd("KEYS")
                .arg(&pattern)
                .query_async(&mut redis)
                .await?;

            for key in keys {
                redis.del::<_, ()>(&key).await?;
            }
        }

        Ok(())
    }

    /// Get user by ID
    pub async fn get_user_by_id(pool: &PgPool, user_id: &Uuid) -> AppResult<Option<User>> {
        UserRepository::find_by_id(pool, user_id).await
    }

    /// Verify JWT token and extract claims
    pub fn verify_token(token: &str, secret: &str) -> AppResult<Claims> {
        let token_data = decode::<Claims>(
            token,
            &DecodingKey::from_secret(secret.as_bytes()),
            &Validation::default(),
        )?;

        Ok(token_data.claims)
    }

    /// Hash password using Argon2
    fn hash_password(password: &str) -> AppResult<String> {
        let salt = SaltString::generate(&mut OsRng);
        let argon2 = Argon2::default();

        let hash = argon2
            .hash_password(password.as_bytes(), &salt)
            .map_err(|e| AppError::Internal(anyhow::anyhow!("Password hashing failed: {}", e)))?
            .to_string();

        Ok(hash)
    }

    /// Verify password against hash
    fn verify_password(password: &str, hash: &str) -> AppResult<bool> {
        let parsed_hash = PasswordHash::new(hash)
            .map_err(|e| AppError::Internal(anyhow::anyhow!("Invalid password hash: {}", e)))?;

        Ok(Argon2::default()
            .verify_password(password.as_bytes(), &parsed_hash)
            .is_ok())
    }

    /// Generate access token
    fn generate_access_token(user: &User, config: &Config) -> AppResult<(String, i64)> {
        let now = Utc::now();
        let expires_at = now + Duration::hours(config.jwt.expiry_hours);
        let expires_in = config.jwt.expiry_hours * 3600;

        let claims = Claims {
            sub: user.id.to_string(),
            username: user.username.clone(),
            role: user.role.clone(),
            exp: expires_at.timestamp(),
            iat: now.timestamp(),
        };

        let token = encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(config.jwt.secret.as_bytes()),
        )
        .map_err(|e| AppError::Internal(anyhow::anyhow!("Token generation failed: {}", e)))?;

        Ok((token, expires_in))
    }

    /// Generate refresh token
    fn generate_refresh_token() -> String {
        Uuid::new_v4().to_string()
    }
}
