use poem_openapi::{Object, Tags};

mod spotify;

pub use spotify::SpotifyApi;

#[derive(Tags)]
enum ApiTags {
    Spotify,
}

#[derive(Object)]
pub struct ErrorResponse {
    pub code: String,
    pub message: String,
    #[oai(skip_serializing_if = "Option::is_none")]
    pub details: Option<String>,
}
