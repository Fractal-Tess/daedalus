use async_trait::async_trait;
use daedalus_core::{DaedalusError, Result};
use daedalus_domain::{
    DownloadDescriptor, FileArtifact, LocalFileState, ModelKind, ModelSummary, PreviewAsset,
    PreviewMediaKind, SearchQuery, SearchResult, SourceInfo, SourceKind, SourceModelBundle,
    SourceVersionBundle,
};
use reqwest::blocking::Client;
use serde::Deserialize;

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
    api_token: Option<String>,
}

impl CivitaiAdapter {
    pub fn new(
        api_base_url: impl Into<String>,
        web_base_url: impl Into<String>,
        enabled: bool,
        api_token: Option<String>,
    ) -> Self {
        Self {
            info: SourceInfo {
                id: "civitai".to_string(),
                kind: SourceKind::Civitai,
                display_name: "Civitai".to_string(),
                api_base_url: api_base_url.into(),
                web_base_url: web_base_url.into(),
                enabled,
            },
            api_token: api_token.filter(|token| !token.trim().is_empty()),
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

    pub fn search_models_blocking(&self, query: SearchQuery) -> Result<SearchResult<ModelSummary>> {
        let client = self.client()?;
        let endpoint = format!("{}/models", self.info.api_base_url.trim_end_matches('/'));
        let response = self
            .with_auth(client.get(endpoint))
            .query(&[("query", query.query), ("limit", query.limit.to_string())])
            .send()
            .map_err(|err| DaedalusError::Http(err.to_string()))?;
        let response = response
            .error_for_status()
            .map_err(|err| DaedalusError::Http(err.to_string()))?;
        let payload: CivitaiModelsResponse = response
            .json()
            .map_err(|err| DaedalusError::Http(format!("failed to decode Civitai search response: {err}")))?;

        Ok(SearchResult {
            total: payload.items.len(),
            next_page: payload.metadata.and_then(|metadata| metadata.next_page),
            items: payload.items.iter().map(|item| self.map_model_summary(item)).collect(),
        })
    }

    pub fn fetch_model_blocking(&self, model_id: &str) -> Result<SourceModelBundle> {
        let client = self.client()?;
        let endpoint = format!(
            "{}/models/{}",
            self.info.api_base_url.trim_end_matches('/'),
            model_id
        );
        let response = self
            .with_auth(client.get(endpoint))
            .send()
            .map_err(|err| DaedalusError::Http(err.to_string()))?;
        let response = response
            .error_for_status()
            .map_err(|err| DaedalusError::Http(err.to_string()))?;
        let payload: CivitaiModel = response
            .json()
            .map_err(|err| DaedalusError::Http(format!("failed to decode Civitai model response: {err}")))?;
        Ok(self.map_model_bundle(&payload))
    }

    pub fn fetch_version_blocking(&self, version_id: &str) -> Result<SourceVersionBundle> {
        let client = self.client()?;
        let endpoint = format!(
            "{}/model-versions/{}",
            self.info.api_base_url.trim_end_matches('/'),
            version_id
        );
        let response = self
            .with_auth(client.get(endpoint))
            .send()
            .map_err(|err| DaedalusError::Http(err.to_string()))?;
        let response = response
            .error_for_status()
            .map_err(|err| DaedalusError::Http(err.to_string()))?;
        let payload: CivitaiVersion = response
            .json()
            .map_err(|err| DaedalusError::Http(format!("failed to decode Civitai version response: {err}")))?;
        Ok(self.map_version_bundle(&payload))
    }

    fn client(&self) -> Result<Client> {
        Client::builder()
            .user_agent("daedalus/0.1.0")
            .build()
            .map_err(|err| DaedalusError::Http(format!("failed to build Civitai client: {err}")))
    }

    fn with_auth(&self, request: reqwest::blocking::RequestBuilder) -> reqwest::blocking::RequestBuilder {
        if let Some(token) = &self.api_token {
            request.bearer_auth(token)
        } else {
            request
        }
    }

    fn map_model_summary(&self, model: &CivitaiModel) -> ModelSummary {
        ModelSummary {
            id: model.id.to_string(),
            title: model.name.clone(),
            creator: model.creator.as_ref().map(|creator| creator.username.clone()),
            model_kind: Self::map_model_type(&model.model_type),
            source_url: Some(format!("{}/models/{}", self.info.web_base_url.trim_end_matches('/'), model.id)),
        }
    }

    fn map_model_bundle(&self, model: &CivitaiModel) -> SourceModelBundle {
        SourceModelBundle {
            model: self.map_model_summary(model),
            description: model.description.clone(),
            tags: model.tags.clone(),
            versions: model
                .model_versions
                .iter()
                .map(|version| self.map_version_bundle(version))
                .collect(),
        }
    }

    fn map_version_bundle(&self, version: &CivitaiVersion) -> SourceVersionBundle {
        SourceVersionBundle {
            id: version.id.to_string(),
            parent_model_id: version.model_id.unwrap_or_default().to_string(),
            version_name: version.name.clone(),
            base_model: version.base_model.clone(),
            files: version.files.iter().map(map_file_artifact).collect(),
            previews: version.images.iter().map(map_preview_asset).collect(),
        }
    }
}

#[async_trait]
impl SourceAdapter for CivitaiAdapter {
    fn source_kind(&self) -> SourceKind {
        SourceKind::Civitai
    }

    async fn health_check(&self) -> Result<SourceHealth> {
        if !self.info.enabled {
            return Ok(SourceHealth {
                ok: false,
                detail: "disabled".to_string(),
            });
        }

        self.search_models_blocking(SearchQuery {
            query: String::new(),
            limit: 1,
        })?;

        Ok(SourceHealth {
            ok: true,
            detail: "reachable".to_string(),
        })
    }

    async fn search_models(&self, query: SearchQuery) -> Result<SearchResult<ModelSummary>> {
        self.search_models_blocking(query)
    }

    async fn fetch_model(&self, model_id: &str) -> Result<SourceModelBundle> {
        self.fetch_model_blocking(model_id)
    }

    async fn fetch_version(&self, version_id: &str) -> Result<SourceVersionBundle> {
        self.fetch_version_blocking(version_id)
    }

    async fn resolve_download(&self, file_id: &str) -> Result<DownloadDescriptor> {
        Ok(DownloadDescriptor {
            source_file_id: file_id.to_string(),
            download_url: format!("{}/download/models/{}", self.info.api_base_url.trim_end_matches('/'), file_id),
            suggested_filename: format!("{file_id}.bin"),
        })
    }
}

#[derive(Debug, Deserialize)]
struct CivitaiModelsResponse {
    items: Vec<CivitaiModel>,
    metadata: Option<CivitaiMetadata>,
}

#[derive(Debug, Deserialize)]
struct CivitaiMetadata {
    #[serde(rename = "nextPage")]
    next_page: Option<String>,
}

#[derive(Debug, Deserialize)]
struct CivitaiCreator {
    username: String,
}

#[derive(Debug, Deserialize)]
struct CivitaiModel {
    id: u64,
    name: String,
    description: Option<String>,
    creator: Option<CivitaiCreator>,
    #[serde(rename = "type")]
    model_type: String,
    #[serde(default)]
    tags: Vec<String>,
    #[serde(default, rename = "modelVersions")]
    model_versions: Vec<CivitaiVersion>,
}

#[derive(Debug, Deserialize)]
struct CivitaiVersion {
    id: u64,
    #[serde(default, rename = "modelId")]
    model_id: Option<u64>,
    name: String,
    #[serde(default, rename = "baseModel")]
    base_model: Option<String>,
    #[serde(default)]
    files: Vec<CivitaiFile>,
    #[serde(default)]
    images: Vec<CivitaiImage>,
}

#[derive(Debug, Deserialize)]
struct CivitaiFile {
    id: u64,
    name: String,
    #[serde(default, rename = "sizeKB")]
    size_kb: Option<f64>,
    #[serde(default)]
    metadata: Option<CivitaiFileMetadata>,
    #[serde(default)]
    hashes: Option<CivitaiHashes>,
    #[serde(default, rename = "downloadUrl")]
    download_url: Option<String>,
}

#[derive(Debug, Deserialize)]
struct CivitaiFileMetadata {
    #[serde(default)]
    format: Option<String>,
    #[serde(default, rename = "fp")]
    precision: Option<String>,
}

#[derive(Debug, Deserialize)]
struct CivitaiHashes {
    #[serde(default, rename = "SHA256")]
    sha256: Option<String>,
}

#[derive(Debug, Deserialize)]
struct CivitaiImage {
    #[serde(default)]
    id: Option<u64>,
    url: String,
    #[serde(default)]
    width: Option<u32>,
    #[serde(default)]
    height: Option<u32>,
    #[serde(rename = "type")]
    media_type: String,
}

fn map_file_artifact(file: &CivitaiFile) -> FileArtifact {
    FileArtifact {
        id: i64::try_from(file.id).unwrap_or(i64::MAX),
        filename: file.name.clone(),
        size_bytes: file.size_kb.map(|size_kb| (size_kb * 1024.0).round() as u64),
        sha256: file.hashes.as_ref().and_then(|hashes| hashes.sha256.clone()),
        mime_type: None,
        format: file.metadata.as_ref().and_then(|metadata| metadata.format.clone()),
        precision: file
            .metadata
            .as_ref()
            .and_then(|metadata| metadata.precision.clone()),
        source_download_url: file.download_url.clone(),
        local_path: None,
        local_state: LocalFileState::Missing,
    }
}

fn map_preview_asset(image: &CivitaiImage) -> PreviewAsset {
    PreviewAsset {
        id: image
            .id
            .and_then(|id| i64::try_from(id).ok())
            .unwrap_or_default(),
        source_url: image.url.clone(),
        local_cached_path: None,
        width: image.width,
        height: image.height,
        media_kind: if image.media_type.eq_ignore_ascii_case("video") {
            PreviewMediaKind::Video
        } else {
            PreviewMediaKind::Image
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_civitai_type_to_internal_kind() {
        assert_eq!(CivitaiAdapter::map_model_type("Checkpoint"), ModelKind::Checkpoint);
        assert_eq!(CivitaiAdapter::map_model_type("TextualInversion"), ModelKind::Embedding);
        assert_eq!(CivitaiAdapter::map_model_type("UnknownThing"), ModelKind::Other);
    }

    #[test]
    fn parses_model_stub_payload() {
        let payload = r#"
        {
          "id": 43331,
          "name": "majicMIX realistic",
          "description": "desc",
          "type": "Checkpoint",
          "tags": ["realistic"],
          "creator": { "username": "Merjic" },
          "modelVersions": [
            {
              "id": 176425,
              "modelId": 43331,
              "name": "v7",
              "baseModel": "SD 1.5",
              "files": [
                {
                  "id": 134792,
                  "name": "majicmixRealistic_v7.safetensors",
                  "sizeKB": 2082642.47,
                  "metadata": { "format": "SafeTensor", "fp": "fp16" },
                  "hashes": { "SHA256": "abc" },
                  "downloadUrl": "https://civitai.com/api/download/models/176425"
                }
              ],
              "images": [
                {
                  "id": 2805533,
                  "url": "https://image.civitai.com/example.jpeg",
                  "width": 1024,
                  "height": 1536,
                  "type": "image"
                }
              ]
            }
          ]
        }
        "#;

        let model: CivitaiModel = serde_json::from_str(payload).expect("parse model");
        let adapter = CivitaiAdapter::new(
            "https://civitai.com/api/v1",
            "https://civitai.com",
            true,
            None,
        );
        let bundle = adapter.map_model_bundle(&model);

        assert_eq!(bundle.model.title, "majicMIX realistic");
        assert_eq!(bundle.versions.len(), 1);
        assert_eq!(bundle.versions[0].files[0].sha256.as_deref(), Some("abc"));
    }
}
