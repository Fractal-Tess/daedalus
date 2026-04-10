use std::fmt::{Display, Formatter};
use std::str::FromStr;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(rename_all = "snake_case")]
pub enum ModelKind {
    Checkpoint,
    Embedding,
    Hypernetwork,
    AestheticGradient,
    LoRA,
    LyCORIS,
    DoRA,
    ControlNet,
    Upscaler,
    Motion,
    Vae,
    Poses,
    Wildcards,
    Workflows,
    Detection,
    Other,
}

impl ModelKind {
    pub const ALL: [Self; 16] = [
        Self::Checkpoint,
        Self::Embedding,
        Self::Hypernetwork,
        Self::AestheticGradient,
        Self::LoRA,
        Self::LyCORIS,
        Self::DoRA,
        Self::ControlNet,
        Self::Upscaler,
        Self::Motion,
        Self::Vae,
        Self::Poses,
        Self::Wildcards,
        Self::Workflows,
        Self::Detection,
        Self::Other,
    ];

    pub fn all() -> &'static [Self] {
        &Self::ALL
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Checkpoint => "Checkpoint",
            Self::Embedding => "Embedding",
            Self::Hypernetwork => "Hypernetwork",
            Self::AestheticGradient => "Aesthetic Gradient",
            Self::LoRA => "LoRA",
            Self::LyCORIS => "LyCORIS",
            Self::DoRA => "DoRA",
            Self::ControlNet => "ControlNet",
            Self::Upscaler => "Upscaler",
            Self::Motion => "Motion",
            Self::Vae => "VAE",
            Self::Poses => "Poses",
            Self::Wildcards => "Wildcards",
            Self::Workflows => "Workflows",
            Self::Detection => "Detection",
            Self::Other => "Other",
        }
    }

    pub fn config_key(self) -> &'static str {
        match self {
            Self::Checkpoint => "checkpoint",
            Self::Embedding => "embedding",
            Self::Hypernetwork => "hypernetwork",
            Self::AestheticGradient => "aesthetic_gradient",
            Self::LoRA => "lora",
            Self::LyCORIS => "lycoris",
            Self::DoRA => "dora",
            Self::ControlNet => "controlnet",
            Self::Upscaler => "upscaler",
            Self::Motion => "motion",
            Self::Vae => "vae",
            Self::Poses => "poses",
            Self::Wildcards => "wildcards",
            Self::Workflows => "workflows",
            Self::Detection => "detection",
            Self::Other => "other",
        }
    }
}

impl Display for ModelKind {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.config_key())
    }
}

impl FromStr for ModelKind {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim().to_ascii_lowercase().as_str() {
            "checkpoint" => Ok(Self::Checkpoint),
            "embedding" => Ok(Self::Embedding),
            "hypernetwork" => Ok(Self::Hypernetwork),
            "aesthetic_gradient" | "aesthetic-gradient" => Ok(Self::AestheticGradient),
            "lora" => Ok(Self::LoRA),
            "lycoris" => Ok(Self::LyCORIS),
            "dora" => Ok(Self::DoRA),
            "controlnet" => Ok(Self::ControlNet),
            "upscaler" => Ok(Self::Upscaler),
            "motion" | "motion_module" | "motion-modules" => Ok(Self::Motion),
            "vae" => Ok(Self::Vae),
            "poses" => Ok(Self::Poses),
            "wildcards" => Ok(Self::Wildcards),
            "workflows" => Ok(Self::Workflows),
            "detection" => Ok(Self::Detection),
            "other" => Ok(Self::Other),
            other => Err(format!("unknown model kind: {other}")),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SourceKind {
    Civitai,
    Local,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum JobKind {
    Download,
    Sync,
    Rescan,
    Import,
    PreviewSync,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum JobStatus {
    Queued,
    Running,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LocalFileState {
    Imported,
    Managed,
    Missing,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PreviewMediaKind {
    Image,
    Video,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SourceRef {
    pub source_kind: SourceKind,
    pub source_model_id: Option<String>,
    pub source_version_id: Option<String>,
    pub source_url: Option<String>,
    pub source_category: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FileArtifact {
    pub id: i64,
    pub filename: String,
    pub size_bytes: Option<u64>,
    pub sha256: Option<String>,
    pub mime_type: Option<String>,
    pub format: Option<String>,
    pub precision: Option<String>,
    pub source_download_url: Option<String>,
    pub local_path: Option<String>,
    pub local_state: LocalFileState,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PreviewAsset {
    pub id: i64,
    pub source_url: String,
    pub local_cached_path: Option<String>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub media_kind: PreviewMediaKind,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LibraryItem {
    pub id: i64,
    pub display_name: String,
    pub primary_model_kind: ModelKind,
    pub source: Option<SourceRef>,
    pub installed_version: Option<String>,
    pub storage_path: String,
    pub favorite: bool,
    pub pinned: bool,
    pub notes: Option<String>,
    pub last_verified_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Job {
    pub id: i64,
    pub kind: JobKind,
    pub status: JobStatus,
    pub progress: f32,
    pub summary: String,
    pub error_summary: Option<String>,
    pub created_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub finished_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SourceInfo {
    pub id: String,
    pub kind: SourceKind,
    pub display_name: String,
    pub api_base_url: String,
    pub web_base_url: String,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SearchQuery {
    pub query: String,
    pub limit: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SearchResult<T> {
    pub items: Vec<T>,
    pub total: usize,
    pub next_page: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ModelSummary {
    pub id: String,
    pub title: String,
    pub creator: Option<String>,
    pub model_kind: ModelKind,
    pub source_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SourceModelBundle {
    pub model: ModelSummary,
    pub description: Option<String>,
    pub tags: Vec<String>,
    pub versions: Vec<SourceVersionBundle>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SourceVersionBundle {
    pub id: String,
    pub parent_model_id: String,
    pub version_name: String,
    pub base_model: Option<String>,
    pub files: Vec<FileArtifact>,
    pub previews: Vec<PreviewAsset>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DownloadDescriptor {
    pub source_file_id: String,
    pub download_url: String,
    pub suggested_filename: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ServiceHealth {
    pub status: String,
    pub mode: String,
    pub library_item_count: usize,
    pub source_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ImportRequest {
    pub path: String,
    pub display_name: Option<String>,
    pub kind: Option<ModelKind>,
    pub copy_into_library: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DownloadRequest {
    pub source_kind: SourceKind,
    pub source_file_id: String,
    pub model_kind: ModelKind,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ConfigUpdateResponse {
    pub saved_to: String,
}
