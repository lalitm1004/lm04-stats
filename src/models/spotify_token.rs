use chrono::{NaiveDateTime, Utc};
use poem_openapi::Object;
use serde::{Deserialize, Serialize};
use sqlx::{Executor, FromRow, Sqlite};
use std::{collections::HashMap, fmt};

use crate::ENV_CONFIG;

#[derive(Debug, FromRow, Serialize, Deserialize, Object)]
pub struct SpotifyToken {
    pub id: i64,
    pub access_token: String,
    pub refresh_token: String,
    pub scope: Option<String>,
    pub expires_at: Option<NaiveDateTime>,
    pub updated_at: Option<NaiveDateTime>,
}

#[derive(Debug, Deserialize)]
struct SpotifyRefreshResponse {
    access_token: String,
    refresh_token: Option<String>,
    expires_in: Option<i64>,
}

impl SpotifyToken {
    const SPOTIFY_TOKEN_URL: &'static str = "https://accounts.spotify.com/api/token";

    pub async fn query_for_token<'e, E>(executor: E) -> Result<Option<Self>, sqlx::Error>
    where
        E: Executor<'e, Database = Sqlite> + Copy,
    {
        sqlx::query_as!(
            SpotifyToken,
            r#"
                SELECT *
                FROM spotify_token
                LIMIT 1
            "#
        )
        .fetch_optional(executor)
        .await
    }

    fn needs_refresh(&self) -> bool {
        match self.expires_at {
            None => true,
            Some(expires_at) => Utc::now().naive_utc() >= expires_at,
        }
    }

    pub async fn get_valid_access_token<'e, E>(executor: E) -> Result<Self, SpotifyTokenError>
    where
        E: Executor<'e, Database = Sqlite> + Copy,
    {
        let token = Self::query_for_token(executor).await?;
        match token {
            Some(token) => {
                let needs_refresh = token.needs_refresh();
                if needs_refresh {
                    Self::refresh_token(executor, &token).await
                } else {
                    Ok(token)
                }
            }
            None => Err(SpotifyTokenError::NoTokenFound),
        }
    }

    pub async fn refresh_token<'e, E>(executor: E, token: &Self) -> Result<Self, SpotifyTokenError>
    where
        E: Executor<'e, Database = Sqlite> + Copy,
    {
        let http_client = reqwest::Client::new();

        let mut params = HashMap::new();
        params.insert("grant_type", "refresh_token");
        params.insert("refresh_token", &token.refresh_token);
        params.insert("client_id", &ENV_CONFIG.spotify_client_id);
        params.insert("client_secret", &ENV_CONFIG.spotify_client_secret);

        let response = http_client
            .post(Self::SPOTIFY_TOKEN_URL)
            .header("Content-Type", "application/x-www-form-urlencoded")
            .form(&params)
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(SpotifyTokenError::RefreshFailed(error_text));
        }

        let refresh_response: SpotifyRefreshResponse = response.json().await?;

        let expires_at = refresh_response
            .expires_in
            .map(|expires_in| Utc::now().naive_utc() + chrono::Duration::seconds(expires_in));

        let new_refresh_token = refresh_response
            .refresh_token
            .clone()
            .unwrap_or_else(|| token.refresh_token.clone());

        let updated_at = Utc::now().naive_utc();

        let updated_token = sqlx::query_as!(
            SpotifyToken,
            r#"
                UPDATE spotify_token
                SET access_token = $1, refresh_token = $2, expires_at = $3, updated_at = $4
                WHERE id = $5
                RETURNING *
            "#,
            refresh_response.access_token,
            new_refresh_token,
            expires_at,
            updated_at,
            token.id
        )
        .fetch_one(executor)
        .await?;

        Ok(updated_token)
    }
}

#[derive(Debug)]
pub enum SpotifyTokenError {
    DatabaseError(sqlx::Error),
    HttpError(reqwest::Error),
    NoTokenFound,
    RefreshFailed(String),
}

impl fmt::Display for SpotifyTokenError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SpotifyTokenError::DatabaseError(e) => write!(f, "Database error: {}", e),
            SpotifyTokenError::HttpError(e) => write!(f, "HTTP error: {}", e),
            SpotifyTokenError::NoTokenFound => write!(f, "No Spotify token found in database"),
            SpotifyTokenError::RefreshFailed(msg) => write!(f, "Token refresh failed: {}", msg),
        }
    }
}

impl std::error::Error for SpotifyTokenError {}

impl From<sqlx::Error> for SpotifyTokenError {
    fn from(err: sqlx::Error) -> Self {
        SpotifyTokenError::DatabaseError(err)
    }
}

impl From<reqwest::Error> for SpotifyTokenError {
    fn from(err: reqwest::Error) -> Self {
        SpotifyTokenError::HttpError(err)
    }
}
