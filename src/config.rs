use once_cell::sync::Lazy;
use std::env;

pub struct EnvConfig {
    pub database_url: String,
    pub spotify_client_id: String,
    pub spotify_client_secret: String,
}

pub static ENV_CONFIG: Lazy<EnvConfig> = Lazy::new(|| {
    dotenvy::dotenv().ok();

    EnvConfig {
        database_url: env::var("DATABASE_URL").expect("DATABASE_URL is not set"),

        spotify_client_id: env::var("SPOTIFY_CLIENT_ID").expect("SPOTIFY_CLIENT_ID is not set"),

        spotify_client_secret: env::var("SPOTIFY_CLIENT_SECRET")
            .expect("SPOTIFY_CLIENT_SECRET is not set"),
    }
});
