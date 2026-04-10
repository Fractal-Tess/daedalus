use async_trait::async_trait;
use daedalus_core::{DaedalusError, Result};
use daedalus_domain::{
    DownloadDescriptor, ModelKind, ModelSummary, SearchQuery, SearchResult, SourceInfo,
    SourceKind, SourceModelBundle, SourceVersionBundle,
};

#[async_trait]
pub trait SourceAdapter: Send + Sync {
    fn source_kind(&self) -> SourceKind;
    async fn health_check(&self) -> Result<SourceHealth>;
    async fn search_models(&self, query: SearchQuery) -> Result<SearchResult<ModelSummary>>;
    async fn fetch_model(&self, model_id: &str) -> Result<SourceModelBundle>;
    async fn fetch_version(&self, version_id: &str) -> Result<SourceVersionBundle>;
    async fn resolve_download(&self, file_id: &str) -> Result<DownloadDescriptor>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceHealth {
    pub ok: bool,
    pub detail: String,
}

#[derive(Debug, Clone)]
pub struct CivitaiAdapter {
    info: SourceInfo,
}

impl CivitaiAdapter {
    pub fn new(api_base_url: impl Into<String>, web_base_url: impl Into<String>, enabled: bool) -> Self {
        Self {
            info: SourceInfo {
                id: "civitai".to_string(),
                kind: SourceKind::Civitai,
                display_name: "Civitai".to_string(),
                api_base_url: api_base_url.into(),
                web_base_url: web_base_url.into(),
                enabled,
            },
        }
    }

    pub fn info(&self) -> SourceInfo {
        self.info.clone()
    }

    pub fn map_model_type(raw: &str) -> ModelKind {
        match raw.trim().to_ascii_lowercase().as_str() {
            "checkpoint" => ModelKind::Checkpoint,
            "textualinversion" | "embedding" => ModelKind::Embedding,
            "hypernetwork" => ModelKind::Hypernetwork,
            "aestheticgradient" => ModelKind::AestheticGradient,
            "lora" => ModelKind::LoRA,
            "lycoris" => ModelKind::LyCORIS,
            "dora" => ModelKind::DoRA,
            "controlnet" => ModelKind::ControlNet,
            "upscaler" => ModelKind::Upscaler,
            "motionmodule" => ModelKind::Motion,
            "vae" => ModelKind::Vae,
            "pose" | "poses" => ModelKind::Poses,
            "wildcards" => ModelKind::Wildcards,
            "workflow" | "workflows" => ModelKind::Workflows,
            "detection" => ModelKind::Detection,
            _ => ModelKind::Other,
        }
    }
}

#[async_trait]
impl SourceAdapter for CivitaiAdapter {
    fn source_kind(&self) -> SourceKind {
        SourceKind::Civitai
    }

    async fn health_check(&self) -> Result<SourceHealth> {
        Ok(SourceHealth {
            ok: self.info.enabled,
            detail: if self.info.enabled {
                "configured".to_string()
            } else {
                "disabled".to_string()
            },
        })
    }

    async fn search_models(&self, _query: SearchQuery) -> Result<SearchResult<ModelSummary>> {
        Ok(SearchResult {
            items: Vec::new(),
            total: 0,
            next_page: None,
        })
    }

    async fn fetch_model(&self, model_id: &str) -> Result<SourceModelBundle> {
        Err(DaedalusError::Other(format!(
            "Civitai model fetch is not implemented yet for model id {model_id}"
        )))
    }

    async fn fetch_version(&self, version_id: &str) -> Result<SourceVersionBundle> {
        Err(DaedalusError::Other(format!(
            "Civitai version fetch is not implemented yet for version id {version_id}"
        )))
    }

    async fn resolve_download(&self, file_id: &str) -> Result<DownloadDescriptor> {
        Err(DaedalusError::Other(format!(
            "Civitai download resolution is not implemented yet for file id {file_id}"
        )))
    }
}
