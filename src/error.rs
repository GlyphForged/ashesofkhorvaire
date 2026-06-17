use askama::Template;
use axum::{
    http::StatusCode,
    response::{Html, IntoResponse, Response},
};

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("not found")]
    NotFound,
    #[error("{0}")]
    Validation(String),
    #[error(transparent)]
    Database(#[from] sqlx::Error),
    #[error(transparent)]
    Template(#[from] askama::Error),
}

#[derive(Template)]
#[template(path = "error.html")]
struct ErrorTemplate<'a> {
    status: u16,
    title: &'a str,
    message: &'a str,
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, title, message) = match &self {
            Self::NotFound => (
                StatusCode::NOT_FOUND,
                "Dossier not found",
                "That page may have moved, been archived, or never existed.",
            ),
            Self::Validation(msg) => (
                StatusCode::UNPROCESSABLE_ENTITY,
                "Check the dossier",
                msg.as_str(),
            ),
            Self::Database(err) => {
                tracing::error!(error = ?err, "database error");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "The archive jammed",
                    "The database could not complete that request.",
                )
            }
            Self::Template(err) => {
                tracing::error!(error = ?err, "template error");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "The archive jammed",
                    "The page could not be rendered.",
                )
            }
        };
        let body = ErrorTemplate {
            status: status.as_u16(),
            title,
            message,
        }
        .render()
        .unwrap_or_else(|_| format!("<h1>{title}</h1><p>{message}</p>"));
        (status, Html(body)).into_response()
    }
}
