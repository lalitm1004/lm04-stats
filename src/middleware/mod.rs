use poem_openapi::{ApiResponse, SecurityScheme, auth::Bearer, payload::Json};

use crate::ENV_CONFIG;
use crate::api::ErrorResponse;

#[allow(dead_code)]
#[derive(SecurityScheme)]
#[oai(
    ty = "bearer",
    key_name = "API_ACCESS_KEY",
    key_in = "header",
    checker = "access_key_checker"
)]
pub struct ApiAuth(String);

#[derive(ApiResponse)]
enum AccessKeyErrorResponse {
    #[oai(status = 401)]
    Unauthorized(Json<ErrorResponse>),
}

async fn access_key_checker(_req: &poem::Request, bearer: Bearer) -> Result<String, poem::Error> {
    let header_data = bearer.token;

    if header_data == ENV_CONFIG.api_access_key {
        Ok(header_data)
    } else {
        Err(AccessKeyErrorResponse::Unauthorized(Json(ErrorResponse {
            code: "UNAUTHORIZED".to_string(),
            message: "Invalid api access key".to_string(),
            details: None,
        }))
        .into())
    }
}
