use daedalus_config::AppConfig;
use daedalus_core::{DaedalusError, Result};
use daedalus_domain::{Job, LibraryItem, ModelSummary, SearchResult, ServiceHealth, SourceModelBundle};

#[derive(Debug, Clone)]
pub struct DaedalusClient {
    base_url: String,
    http: reqwest::blocking::Client,
}

impl DaedalusClient {
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into().trim_end_matches('/').to_string(),
            http: reqwest::blocking::Client::new(),
        }
    }

    pub fn health(&self) -> Result<ServiceHealth> {
        self.get("/health")
    }

    pub fn get_config(&self) -> Result<AppConfig> {
        self.get("/config")
    }

    pub fn update_config(&self, config: &AppConfig) -> Result<()> {
        let url = format!("{}{}", self.base_url, "/config");
        let response = self
            .http
            .put(url)
            .json(config)
            .send()
            .map_err(|err| DaedalusError::Http(err.to_string()))?;
        if !response.status().is_success() {
            return Err(DaedalusError::Http(format!(
                "request failed with status {}",
                response.status()
            )));
        }
        Ok(())
    }

    pub fn list_library_items(&self) -> Result<Vec<LibraryItem>> {
        self.get("/library/items")
    }

    pub fn list_jobs(&self) -> Result<Vec<Job>> {
        self.get("/jobs")
    }

    pub fn rescan_library(&self) -> Result<Job> {
        self.post_empty("/library/rescan")
    }

    pub fn search_civitai_models(&self, query: &str, limit: usize) -> Result<SearchResult<ModelSummary>> {
        let url = format!("{}{}", self.base_url, "/sources/civitai/search");
        let response = self
            .http
            .get(url)
            .query(&[("q", query), ("limit", &limit.to_string())])
            .send()
            .map_err(|err| DaedalusError::Http(err.to_string()))?;
        if !response.status().is_success() {
            return Err(DaedalusError::Http(format!(
                "request failed with status {}",
                response.status()
            )));
        }
        response
            .json()
            .map_err(|err| DaedalusError::Http(format!("failed to decode response: {err}")))
    }

    pub fn fetch_civitai_model(&self, model_id: &str) -> Result<SourceModelBundle> {
        self.get(&format!("/sources/civitai/models/{model_id}"))
    }

    fn get<T: serde::de::DeserializeOwned>(&self, path: &str) -> Result<T> {
        let url = format!("{}{}", self.base_url, path);
        let response = self
            .http
            .get(url)
            .send()
            .map_err(|err| DaedalusError::Http(err.to_string()))?;
        if !response.status().is_success() {
            return Err(DaedalusError::Http(format!(
                "request failed with status {}",
                response.status()
            )));
        }
        response
            .json()
            .map_err(|err| DaedalusError::Http(format!("failed to decode response: {err}")))
    }

    fn post_empty<T: serde::de::DeserializeOwned>(&self, path: &str) -> Result<T> {
        let url = format!("{}{}", self.base_url, path);
        let response = self
            .http
            .post(url)
            .send()
            .map_err(|err| DaedalusError::Http(err.to_string()))?;
        if !response.status().is_success() {
            return Err(DaedalusError::Http(format!(
                "request failed with status {}",
                response.status()
            )));
        }
        response
            .json()
            .map_err(|err| DaedalusError::Http(format!("failed to decode response: {err}")))
    }
}
