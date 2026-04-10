use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use sanitize_filename::sanitize;
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub type Result<T> = std::result::Result<T, DaedalusError>;

#[derive(Debug, Error)]
pub enum DaedalusError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("config error: {0}")]
    Config(String),
    #[error("database error: {0}")]
    Database(String),
    #[error("http error: {0}")]
    Http(String),
    #[error("not found: {0}")]
    NotFound(String),
    #[error("validation error: {0}")]
    Validation(String),
    #[error("{0}")]
    Other(String),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Pagination {
    pub limit: usize,
    pub offset: usize,
}

pub fn now_utc() -> DateTime<Utc> {
    Utc::now()
}

pub fn sanitize_path_component(value: &str) -> String {
    let sanitized = sanitize(value);
    let trimmed = sanitized.trim_matches('.');
    if trimmed.is_empty() {
        "unnamed".to_string()
    } else {
        trimmed.to_string()
    }
}

pub fn normalize_path(path: &Path) -> PathBuf {
    match path.canonicalize() {
        Ok(canonical) => canonical,
        Err(_) => path.to_path_buf(),
    }
}

pub fn supported_model_extensions() -> &'static [&'static str] {
    &[
        "bin", "ckpt", "gguf", "json", "onnx", "pt", "pth", "safetensors", "txt", "vae", "yaml",
        "yml",
    ]
}

pub fn looks_like_model_file(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| supported_model_extensions().contains(&ext.to_ascii_lowercase().as_str()))
        .unwrap_or(false)
}
