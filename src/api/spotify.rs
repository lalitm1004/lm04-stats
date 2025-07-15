use poem::{Result, web::Data};
use poem_openapi::{ApiResponse, Object, OpenApi, payload::Json};
use serde::{Deserialize, Serialize};

use super::{ApiTags, ErrorResponse};
use crate::{AppState, middleware::ApiAuth, models::SpotifyToken};

#[derive(Debug, Serialize, Deserialize, Object)]
pub struct TrackDetails {
    pub item: Option<Track>,
    pub is_playing: bool,
    pub played_at: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Object)]
pub struct Track {
    pub id: String,
    pub name: String,
    pub album: Album,
    pub artists: Vec<Artist>,
    pub explicit: bool,
    pub preview_url: Option<String>,
    pub duration_ms: u64,
    pub popularity: u8,
}

#[derive(Debug, Serialize, Deserialize, Object)]
pub struct Album {
    pub id: String,
    pub name: String,
    pub artists: Vec<Artist>,
    pub images: Vec<AlbumImage>,
}

#[derive(Debug, Serialize, Deserialize, Object)]
pub struct AlbumImage {
    pub url: String,
    pub height: Option<usize>,
    pub width: Option<usize>,
}

#[derive(Debug, Serialize, Deserialize, Object)]
pub struct Artist {
    pub id: String,
    pub name: String,
}

#[derive(ApiResponse)]
enum TrackWidgetResponse {
    #[oai(status = 200)]
    Ok(Json<TrackDetails>),

    #[oai(status = 401)]
    Unauthorized(Json<ErrorResponse>),

    #[oai(status = 500)]
    InternalServerError(Json<ErrorResponse>),
}

pub struct SpotifyApi;

#[OpenApi(tag = "ApiTags::Spotify")]
impl SpotifyApi {
    const SPOTIFY_API_BASE_URL: &'static str = "https://api.spotify.com/v1";

    #[oai(path = "/api/spotify/track-widget", method = "get")]
    async fn get_currently_playing(
        &self,
        state: Data<&AppState>,
        _api_access_key: ApiAuth,
    ) -> Result<TrackWidgetResponse> {
        let token = match SpotifyToken::get_valid_access_token(&*state.db).await {
            Ok(token) => token,
            Err(e) => {
                eprintln!("Failed to get valid access token: {}", e);
                return Ok(TrackWidgetResponse::Unauthorized(Json(ErrorResponse {
                    code: "SPOTIFY_AUTH_FAILED".to_string(),
                    message: "Failed to authenticate with Spotify".to_string(),
                    details: None,
                })));
            }
        };

        let http_client = reqwest::Client::new();

        let currently_playing_response = match self
            .fetch_currently_playing_track(&http_client, &token.access_token)
            .await
        {
            Ok(response) => response,
            Err(e) => {
                eprintln!("Failed to fetch currently playing track: {}", e);
                return Ok(TrackWidgetResponse::InternalServerError(Json(
                    ErrorResponse {
                        code: "SPOTIFY_API_ERROR".to_string(),
                        message: "Failed to connect to Spotify API".to_string(),
                        details: None,
                    },
                )));
            }
        };

        match currently_playing_response.status() {
            reqwest::StatusCode::OK => {
                match currently_playing_response.json::<serde_json::Value>().await {
                    Ok(json) => {
                        let currently_playing = Self::parse_currently_playing_response(&json);

                        // Check if we have a track or if it's an episode/unsupported type
                        if currently_playing.item.is_some() {
                            Ok(TrackWidgetResponse::Ok(Json(currently_playing)))
                        } else {
                            // Currently playing is an episode or unsupported type, fallback to recently played
                            self.handle_no_track_playing(&http_client, &token.access_token)
                                .await
                        }
                    }
                    Err(e) => {
                        eprintln!("Failed to parse Spotify response: {}", e);
                        Ok(TrackWidgetResponse::InternalServerError(Json(
                            ErrorResponse {
                                code: "SPOTIFY_RESPONSE_PARSE_FAILURE".to_string(),
                                message: "Failed to parse Spotify response".to_string(),
                                details: None,
                            },
                        )))
                    }
                }
            }
            reqwest::StatusCode::NO_CONTENT => {
                // Nothing currently playing, fallback to recently played
                self.handle_no_track_playing(&http_client, &token.access_token)
                    .await
            }
            _ => {
                eprintln!(
                    "Unexpected response status: {}",
                    currently_playing_response.status()
                );
                Ok(TrackWidgetResponse::InternalServerError(Json(
                    ErrorResponse {
                        code: "SPOTIFY_UNEXPECTED_RESPONSE".to_string(),
                        message: "Unexpected response from Spotify API".to_string(),
                        details: None,
                    },
                )))
            }
        }
    }

    async fn fetch_currently_playing_track(
        &self,
        http_client: &reqwest::Client,
        access_token: &str,
    ) -> Result<reqwest::Response, reqwest::Error> {
        http_client
            .get(&format!(
                "{}/me/player/currently-playing?market=IN",
                Self::SPOTIFY_API_BASE_URL
            ))
            .header("Authorization", format!("Bearer {}", access_token))
            .send()
            .await
    }

    async fn handle_no_track_playing(
        &self,
        http_client: &reqwest::Client,
        access_token: &str,
    ) -> Result<TrackWidgetResponse> {
        match self
            .fetch_recently_played_track(http_client, access_token)
            .await
        {
            Ok(recently_played) => Ok(TrackWidgetResponse::Ok(Json(recently_played))),
            Err(e) => {
                eprintln!("Failed to fetch recently played track: {}", e);
                // Return empty response instead of error for better UX
                Ok(TrackWidgetResponse::Ok(Json(TrackDetails {
                    item: None,
                    is_playing: false,
                    played_at: None,
                })))
            }
        }
    }

    async fn fetch_recently_played_track(
        &self,
        http_client: &reqwest::Client,
        access_token: &str,
    ) -> Result<TrackDetails, Box<dyn std::error::Error>> {
        let response = http_client
            .get(&format!(
                "{}/me/player/recently-played?limit=1&market=IN",
                Self::SPOTIFY_API_BASE_URL
            ))
            .header("Authorization", format!("Bearer {}", access_token))
            .send()
            .await?;

        match response.status() {
            reqwest::StatusCode::OK => {
                let json = response.json::<serde_json::Value>().await?;
                let recently_played = Self::parse_recently_played_response(&json);
                Ok(recently_played)
            }
            _ => Err(format!(
                "Failed to fetch recently played tracks: {}",
                response.status()
            )
            .into()),
        }
    }

    fn parse_currently_playing_response(json: &serde_json::Value) -> TrackDetails {
        let is_playing = json["is_playing"].as_bool().unwrap_or(false);

        let item = if let Some(item_data) = json["item"].as_object() {
            if item_data["type"].as_str() == Some("track") {
                Some(Self::parse_track_from_json(item_data))
            } else {
                // Skip non-track items (episodes, podcasts, etc.)
                None
            }
        } else {
            None
        };

        TrackDetails {
            item,
            is_playing,
            played_at: None,
        }
    }

    fn parse_recently_played_response(json: &serde_json::Value) -> TrackDetails {
        let empty_vec = vec![];
        let items = json["items"].as_array().unwrap_or(&empty_vec);

        if items.is_empty() {
            return TrackDetails {
                item: None,
                is_playing: false,
                played_at: None,
            };
        }

        let most_recent_item = &items[0];
        let played_at = most_recent_item["played_at"]
            .as_str()
            .map(|s| s.to_string());

        let item = most_recent_item["track"]
            .as_object()
            .map(|track_data| Self::parse_track_from_json(track_data));

        TrackDetails {
            item,
            is_playing: false,
            played_at,
        }
    }

    fn parse_track_from_json(track_data: &serde_json::Map<String, serde_json::Value>) -> Track {
        Track {
            id: track_data["id"].as_str().unwrap_or("").to_string(),
            name: track_data["name"].as_str().unwrap_or("").to_string(),
            album: Self::parse_album_from_json(&track_data["album"]),
            artists: Self::parse_artists_from_json(&track_data["artists"]),
            explicit: track_data["explicit"].as_bool().unwrap_or(false),
            preview_url: track_data["preview_url"].as_str().map(|s| s.to_string()),
            duration_ms: track_data["duration_ms"].as_u64().unwrap_or(0),
            popularity: track_data["popularity"].as_u64().unwrap_or(0) as u8,
        }
    }

    fn parse_album_from_json(album_data: &serde_json::Value) -> Album {
        Album {
            id: album_data["id"].as_str().unwrap_or("").to_string(),
            name: album_data["name"].as_str().unwrap_or("").to_string(),
            artists: Self::parse_artists_from_json(&album_data["artists"]),
            images: Self::parse_album_images_from_json(&album_data["images"]),
        }
    }

    fn parse_artists_from_json(artists_value: &serde_json::Value) -> Vec<Artist> {
        artists_value
            .as_array()
            .unwrap_or(&vec![])
            .iter()
            .map(|artist| Artist {
                id: artist["id"].as_str().unwrap_or("").to_string(),
                name: artist["name"].as_str().unwrap_or("").to_string(),
            })
            .collect()
    }

    fn parse_album_images_from_json(images_value: &serde_json::Value) -> Vec<AlbumImage> {
        images_value
            .as_array()
            .unwrap_or(&vec![])
            .iter()
            .map(|image| AlbumImage {
                url: image["url"].as_str().unwrap_or("").to_string(),
                height: image["height"].as_u64().map(|h| h as usize),
                width: image["width"].as_u64().map(|w| w as usize),
            })
            .collect()
    }
}
