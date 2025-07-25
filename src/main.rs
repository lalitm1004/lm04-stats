use poem::{EndpointExt, Route, Server, listener::TcpListener, middleware::Cors};
use poem_openapi::OpenApiService;
use std::{process::ExitCode, sync::Arc};

use lm04_stats::{AppState, SpotifyApi, create_db_pool};

#[tokio::main]
async fn main() -> ExitCode {
    let db_pool = create_db_pool().await;
    let state = AppState {
        db: Arc::new(db_pool),
    };

    let api_service =
        OpenApiService::new(SpotifyApi, "lm04-stats", "1.0").server("http://localhost:3000");
    let ui = api_service.swagger_ui();

    let cors = Cors::new().allow_origins_fn(|origin: &str| {
        let origin = origin.to_ascii_lowercase();

        if origin.ends_with(".lm04.me") || origin == "https://lm04.me" {
            return true;
        }

        false
    });

    let app = Route::new()
        .nest("/", api_service)
        .nest("/docs", ui)
        .data(state)
        .with(cors);

    let _ = Server::new(TcpListener::bind("127.0.0.1:3000"))
        .run(app)
        .await;

    ExitCode::FAILURE
}
