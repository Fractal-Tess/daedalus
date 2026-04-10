# Daedalus Specification

## Purpose

Daedalus is a modular Rust application for discovering, downloading, cataloging, and serving AI model artifacts. It should work in two primary modes:

- as a standalone desktop app with an `egui` user interface
- as a long-running daemon with an API that a local or remote GUI can connect to

The system is intended to manage model files on disk, keep structured metadata for those files, persist preview images and related media, and support multiple model sources over time. The first external source is Civitai.

## Product Goals

- Provide a clean local library for AI models and related assets.
- Support configurable storage roots so files are not tied to one machine layout.
- Keep a durable catalog of metadata independent of the source website.
- Make source integrations modular so Civitai is only one adapter among many.
- Support both embedded single-process usage and client/server usage.
- Treat downloads, file moves, thumbnail caching, and metadata sync as first-class operations.
- Support multiple model categories such as checkpoint, embedding, LoRA, ControlNet, VAE, workflows, and similar asset classes.

## Non-Goals For V1

- Training models.
- Running inference.
- Editing model weights.
- Full social/community features from source sites.
- Multi-user auth/permissions beyond basic local or trusted-network deployment.

## External Source Intelligence

Initial Civitai observations gathered with `agent-browser`:

- Public web navigation includes `Models`, search, sorting, and filter controls.
- The models area exposes categorization and discovery affordances that imply a structured source taxonomy.
- The site includes public preview imagery and high-cardinality metadata on listing pages.
- The footer exposes an `API` entry point, so the source adapter should assume an API-backed ingestion path rather than relying on HTML scraping.

Design implication:

- Daedalus should use official or stable API endpoints whenever possible.
- Browser scraping should be a fallback only for fields unavailable in the API.
- All source-derived fields should be normalized into an internal schema so the rest of the app is source-agnostic.

## High-Level Architecture

Daedalus should be a Cargo workspace with a small set of focused crates.

### Proposed Workspace Layout

```text
daedalus/
  Cargo.toml
  crates/
    daedalus-core/
    daedalus-config/
    daedalus-db/
    daedalus-domain/
    daedalus-sources/
    daedalus-downloads/
    daedalus-service/
    daedalus-api/
    daedalus-daemon/
    daedalus-gui/
    daedalus-client/
    daedalus-cli/
```

### Crate Responsibilities

#### `daedalus-domain`

Pure domain types and enums:

- model identifiers
- model kinds
- source references
- file records
- image and preview metadata
- library items
- tags
- statuses
- sync/download events

This crate should avoid runtime-heavy dependencies.

#### `daedalus-config`

Configuration loading, validation, migration, and persistence:

- config file discovery
- defaults
- schema versioning
- path expansion
- environment variable overrides
- profile management if added later

#### `daedalus-db`

Persistence layer for the local catalog:

- schema definitions and migrations
- repository/query layer
- transaction boundaries
- indexing strategy

Recommended V1 approach:

- `SQLite` for metadata
- `sqlx` or `rusqlite` for access

#### `daedalus-core`

Cross-cutting utility logic shared by service, API, and GUI:

- ids
- error types
- time helpers
- path helpers
- hashing
- serialization helpers

#### `daedalus-sources`

Source adapter framework and concrete integrations:

- `SourceAdapter` trait
- Civitai adapter
- source capability metadata
- rate limit and pagination handling
- source-to-domain normalization

This crate should make it easy to add later adapters for Hugging Face, direct URLs, local imports, or ComfyUI model folders.

#### `daedalus-downloads`

Download orchestration:

- queueing
- resumable downloads if supported
- checksum verification
- temp file handling
- final placement into managed storage
- cancellation and progress reporting

#### `daedalus-service`

Application service layer used by both embedded mode and daemon mode:

- library operations
- sync operations
- source search
- model import/export
- preview caching
- background job coordination

This is the main business-logic crate.

#### `daedalus-api`

HTTP and optionally WebSocket API surface:

- route definitions
- DTOs separate from domain types when useful
- API versioning
- streaming job/progress updates
- health/status endpoints

Recommended V1 stack:

- `axum`
- `tokio`
- `serde`

#### `daedalus-daemon`

Executable that hosts:

- config
- database
- service layer
- background workers
- API server

It should be suitable for systemd user services and long-running local operation.

#### `daedalus-client`

Transport client for talking to a daemon:

- API bindings
- retry logic
- streaming subscriptions
- local/remote endpoint handling

The GUI should depend on this crate when running in remote mode.

#### `daedalus-gui`

`egui` desktop application:

- local embedded mode
- daemon-connected mode
- views for discovery, library, downloads, sources, settings, and model detail

Recommended V1 stack:

- `eframe` + `egui`

#### `daedalus-cli`

Optional but useful operational CLI:

- start daemon
- inspect config
- trigger sync
- import local files
- query library

## Runtime Modes

Daedalus should support three execution modes.

### 1. Embedded Desktop Mode

The GUI starts an in-process service stack:

- local config
- local SQLite catalog
- local background workers
- no external daemon required

Use case:

- single-user desktop app

### 2. Daemon + GUI On Same Machine

The daemon runs independently and the GUI connects over local HTTP:

- daemon owns background jobs and library state
- GUI is a thin client
- daemon can continue downloads after the GUI closes

Use case:

- laptop or workstation with persistent model management

### 3. Remote GUI + Remote Daemon

The GUI can connect to another machine:

- browsing remote library
- managing downloads on a server or NAS-attached machine
- using the same UI regardless of process topology

Use case:

- headless model host
- shared home-lab machine

## Configuration

Configuration must be persisted outside the binary and must be user-editable.

### Storage Location

Recommended default:

- Linux: `${XDG_CONFIG_HOME:-~/.config}/daedalus/config.toml`
- macOS: `~/Library/Application Support/daedalus/config.toml`
- Windows: `%APPDATA%/daedalus/config.toml`

Cache and state:

- cache: `${XDG_CACHE_HOME:-~/.cache}/daedalus`
- data/state: `${XDG_DATA_HOME:-~/.local/share}/daedalus`

### Config Principles

- Human-editable `TOML`
- Explicit schema version
- Clear defaults
- Path overrides for all managed storage roots
- Safe startup validation with actionable errors

### Initial Config Shape

```toml
version = 1

[library]
default_storage_root = "/models"
temp_download_root = "/models/.tmp"
preview_cache_root = "/models/.previews"
managed_by_kind = true
deduplicate_by_sha256 = true

[database]
path = "~/.local/share/daedalus/daedalus.db"

[daemon]
enabled = true
host = "127.0.0.1"
port = 4590

[gui]
default_mode = "auto"
remote_url = "http://127.0.0.1:4590"

[sources.civitai]
enabled = true
api_base_url = "https://civitai.com/api/v1"
web_base_url = "https://civitai.com"
api_token = ""
sync_preview_images = true
sync_creator_metadata = true

[model_paths]
checkpoint = "/models/checkpoints"
embedding = "/models/embeddings"
hypernetwork = "/models/hypernetworks"
aesthetic_gradient = "/models/aesthetic-gradients"
lora = "/models/loras"
lycoris = "/models/lycoris"
dora = "/models/dora"
controlnet = "/models/controlnet"
upscaler = "/models/upscalers"
motion = "/models/motion-modules"
vae = "/models/vae"
poses = "/models/poses"
wildcards = "/models/wildcards"
workflows = "/models/workflows"
detection = "/models/detection"
other = "/models/other"
```

### Config Behavior

- If `managed_by_kind = true`, new downloads default into kind-specific paths.
- Paths may be absolute or resolved relative to `default_storage_root`.
- The app should support editing config from the GUI and persisting it back to disk.
- Validation should detect overlapping or missing directories and report clear remediation.
- Secrets like source API tokens should be allowed in config initially, but V2 should support OS keyring integration.

## Model Taxonomy

The app should maintain an internal normalized `ModelKind` enum. V1 should include the kinds shown in your UI direction:

- `Checkpoint`
- `Embedding`
- `Hypernetwork`
- `AestheticGradient`
- `LoRA`
- `LyCORIS`
- `DoRA`
- `ControlNet`
- `Upscaler`
- `Motion`
- `VAE`
- `Poses`
- `Wildcards`
- `Workflows`
- `Detection`
- `Other`

### Taxonomy Rules

- Internal kinds are stable and source-agnostic.
- Each source adapter maps its native categories into `ModelKind`.
- If a source exposes a category Daedalus does not yet know, ingest it as `Other` plus preserve the original source category string.
- A library item may have one primary kind and optional secondary tags.

## Data Model

Daedalus should separate source records from local library records.

### Core Entities

#### Source

- id
- kind
- display name
- base URLs
- enabled flag

#### Source Model

Represents a source-side object such as a Civitai model:

- source model id
- source url
- title
- creator
- description
- model kind
- tags
- nsfw/safety flags
- created/updated timestamps

#### Source Model Version

- source version id
- parent source model id
- version name
- base model information
- source-published files
- preview images
- stats if available

#### File Artifact

Represents a local or remote file candidate:

- id
- filename
- size
- sha256
- mime type
- format
- precision if known
- source download url
- local path
- local state

#### Preview Asset

- id
- source url
- local cached path
- width
- height
- media kind
- blurhash or placeholder support later

#### Library Item

Normalized top-level local record:

- internal id
- display name
- primary model kind
- source provenance
- installed version
- storage path
- favorite/pinned flags
- notes
- last verified timestamp

#### Job

Background work record:

- id
- kind
- status
- progress
- created at
- started at
- finished at
- error summary

## Persistence Strategy

Use SQLite as the source of truth for:

- library records
- source metadata snapshots
- download jobs
- file inventories
- preview cache records
- settings derived from config if needed for UI convenience

Use the filesystem for:

- downloaded model files
- preview images and thumbnails
- exported manifests
- logs if file logging is enabled

### Suggested Tables

- `sources`
- `source_models`
- `source_model_versions`
- `source_files`
- `source_previews`
- `library_items`
- `library_item_files`
- `jobs`
- `job_events`
- `config_snapshots` optional

## API Design

The daemon API should expose resource-oriented endpoints and event streaming.

### V1 Endpoint Areas

- `GET /health`
- `GET /config`
- `PUT /config`
- `GET /library/items`
- `GET /library/items/:id`
- `POST /library/import`
- `POST /library/rescan`
- `GET /sources`
- `GET /sources/civitai/search`
- `GET /sources/civitai/models/:id`
- `POST /downloads`
- `POST /downloads/:id/cancel`
- `GET /jobs`
- `GET /jobs/:id`
- `GET /events` for SSE or WebSocket updates

### API Requirements

- JSON over HTTP for normal CRUD
- SSE or WebSocket for job progress
- Versioned under `/api/v1` once the surface stabilizes
- Same service contracts should be callable in-process without HTTP overhead

## GUI Requirements

The GUI should be written with `egui` and keep rendering concerns separate from service logic.

### Primary Views

- library
- discovery/search
- downloads/jobs
- model detail
- settings
- source management

### Library View

- filter by `ModelKind`
- search by title, creator, tags, filename
- sort by recent, installed size, source, favorites
- show installed state and file location

### Discovery View

- search Civitai
- filter by source-native constraints where supported
- inspect a model before downloading
- choose a specific version and file
- preview associated images

### Downloads View

- active queue
- progress
- retries
- failure reasons
- open containing folder

### Settings View

- edit storage roots
- edit daemon connection settings
- choose embedded or remote mode
- manage source tokens
- test source connectivity

### UX Mode Behavior

- On startup, GUI mode `auto` should attempt daemon connection first and fall back to embedded mode if unavailable.
- The current connection mode must be visible in the UI.
- The GUI should support reconnecting to a remote daemon without restart.

## Source Adapter Design

Each source adapter should implement a common trait.

### Proposed Trait Shape

```rust
pub trait SourceAdapter {
    fn source_kind(&self) -> SourceKind;
    async fn health_check(&self) -> Result<SourceHealth>;
    async fn search_models(&self, query: SearchQuery) -> Result<SearchResult<ModelSummary>>;
    async fn fetch_model(&self, model_id: SourceModelId) -> Result<SourceModelBundle>;
    async fn fetch_version(&self, version_id: SourceVersionId) -> Result<SourceVersionBundle>;
    async fn resolve_download(&self, file_id: SourceFileId) -> Result<DownloadDescriptor>;
}
```

### Civitai Adapter Requirements

- search models
- fetch model detail and versions
- fetch preview image metadata
- fetch downloadable file metadata
- normalize source categories to internal `ModelKind`
- store original source ids and URLs for provenance

### Civitai Adapter Notes

- Prefer API calls over HTML scraping.
- Support optional authenticated requests if a token is configured.
- Keep adapter-specific DTOs private to the source crate.
- Normalize source-side tags and base model labels into internal fields plus raw metadata blobs.

## File Management

Daedalus is fundamentally a file manager for model artifacts. File placement rules need to be deterministic.

### Placement Rules

- Every completed download first lands in `temp_download_root`.
- Files are verified before moving into their final managed path.
- Final path is computed from config, model kind, and selected version/file metadata.
- Name collisions should be handled predictably, preferably with suffixing or content-hash-based disambiguation.
- Optionally store a sidecar manifest next to each installed file in V2.

### Example Final Path Strategy

```text
{model_paths[kind]}/{sanitized_model_name}/{sanitized_version_name}/{filename}
```

Alternative flat mode may be added later for compatibility with existing toolchains.

## Background Jobs

The daemon and embedded service should run background jobs for:

- downloads
- metadata refresh
- preview image sync
- filesystem rescan
- orphan detection

### Job Requirements

- persisted job records
- resumable state where practical
- progress events for GUI
- cancellation support
- clear failure summaries

## Local Rescan And Import

Daedalus should not assume all files were downloaded by itself.

### V1 Support

- import arbitrary local files into the catalog
- rescan configured model directories
- infer `ModelKind` from directory mapping and file extensions where possible
- attach manually imported files to user-created library records if source metadata is unavailable

This matters because users often already have existing model directories.

## Modularity Rules

To keep the workspace maintainable:

- GUI must not directly depend on database internals.
- API must talk to the service layer, not directly to source adapters or SQL.
- Source adapters must not know about `egui`.
- Domain types should not depend on HTTP frameworks.
- Embedded mode must reuse the same service crate as daemon mode.

## Suggested Dependency Direction

```text
daedalus-domain
  <- daedalus-core
  <- daedalus-config
  <- daedalus-db
  <- daedalus-sources
  <- daedalus-downloads
  <- daedalus-service
  <- daedalus-api
  <- daedalus-client
  <- daedalus-daemon
  <- daedalus-gui
  <- daedalus-cli
```

`daedalus-service` is the center of the runtime architecture.

## V1 Milestones

### Milestone 1: Skeleton

- create workspace
- define core/domain/config crates
- define `ModelKind`
- create config loading and persistence
- establish SQLite schema

### Milestone 2: Local Library

- local file import
- library browsing in `egui`
- settings editor for storage roots
- basic rescan support

### Milestone 3: Civitai Integration

- source adapter
- search UI
- model detail UI
- download queue
- preview image caching

### Milestone 4: Daemon/API Split

- daemon executable
- API routes
- remote GUI connection
- background jobs persisting independent of GUI lifetime

## Open Questions

- Which Civitai API endpoints and auth scopes are required for all desired downloads?
- Should downloaded previews be stored under cache or library-managed roots?
- Do you want compatibility path presets for existing tools like ComfyUI or A1111?
- Should remote daemon mode support TLS and auth in V1, or stay trusted-network only?
- Should workflows and non-binary assets share the same library UX as binary model files, or get a specialized detail panel?

## Recommended Next Step

After this spec, the next implementation task should be:

1. create the Cargo workspace
2. scaffold the crates listed above
3. implement `daedalus-domain`, `daedalus-config`, and `daedalus-service` first
4. keep the first GUI running in embedded mode before adding remote-daemon support
