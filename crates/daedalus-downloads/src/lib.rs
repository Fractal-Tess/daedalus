use std::path::PathBuf;

use daedalus_core::sanitize_path_component;
use daedalus_domain::ModelKind;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlacementInput<'a> {
    pub root: &'a std::path::Path,
    pub kind: ModelKind,
    pub model_name: &'a str,
    pub version_name: &'a str,
    pub filename: &'a str,
}

pub fn compute_managed_path(input: PlacementInput<'_>) -> PathBuf {
    input
        .root
        .join(input.kind.config_key())
        .join(sanitize_path_component(input.model_name))
        .join(sanitize_path_component(input.version_name))
        .join(sanitize_path_component(input.filename))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn computes_deterministic_path() {
        let path = compute_managed_path(PlacementInput {
            root: std::path::Path::new("/models"),
            kind: ModelKind::Checkpoint,
            model_name: "Deliberate Model",
            version_name: "v1.0",
            filename: "model.safetensors",
        });
        assert_eq!(
            path.to_string_lossy(),
            "/models/checkpoint/Deliberate Model/v1.0/model.safetensors"
        );
    }
}
