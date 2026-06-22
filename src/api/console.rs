use std::path::PathBuf;

use axum::{
    extract::Path,
    http::{StatusCode, header::CONTENT_TYPE},
    response::{Html, IntoResponse, Response},
};

const CONSOLE_HTML: &str = r#"<!doctype html>
<html lang="en">
  <head>
    <meta charset="utf-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1" />
    <title>OpenDaemon Console</title>
  </head>
  <body>
    <div id="root">
      <h1>OpenDaemon Console</h1>
      <p>Build the web Console assets to enable the full local UI.</p>
    </div>
  </body>
</html>
"#;

pub async fn shell() -> Response {
    console_index_response()
}

pub async fn serve(Path(path): Path<String>) -> Response {
    let sanitized = sanitize_path(&path);
    let dist = console_dist_dir();
    let asset = sanitized
        .as_ref()
        .map(|path| dist.join(path))
        .filter(|path| path.is_file());

    if let Some(asset) = asset {
        return match std::fs::read(&asset) {
            Ok(bytes) => (
                StatusCode::OK,
                [(CONTENT_TYPE, content_type(&asset))],
                bytes,
            )
                .into_response(),
            Err(_) => StatusCode::NOT_FOUND.into_response(),
        };
    }

    console_index_response()
}

fn console_index_response() -> Response {
    match std::fs::read_to_string(console_dist_dir().join("index.html")) {
        Ok(index) => Html(index).into_response(),
        Err(_) => Html(CONSOLE_HTML).into_response(),
    }
}

fn sanitize_path(path: &str) -> Option<PathBuf> {
    let trimmed = path.trim_start_matches('/');
    if trimmed.is_empty()
        || trimmed
            .split('/')
            .any(|segment| segment.is_empty() || segment == "." || segment == "..")
    {
        return None;
    }
    Some(PathBuf::from(trimmed))
}

fn console_dist_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("console/dist")
}

pub(crate) fn content_type(path: &std::path::Path) -> &'static str {
    match path.extension().and_then(|extension| extension.to_str()) {
        Some("css") => "text/css; charset=utf-8",
        Some("js") => "text/javascript; charset=utf-8",
        Some("mjs") => "text/javascript; charset=utf-8",
        Some("wasm") => "application/wasm",
        Some("json") => "application/json; charset=utf-8",
        Some("svg") => "image/svg+xml",
        Some("png") => "image/png",
        Some("webp") => "image/webp",
        Some("ico") => "image/x-icon",
        Some("html") => "text/html; charset=utf-8",
        _ => "application/octet-stream",
    }
}
