mod detect;
mod loader;
mod schema;
mod service;
mod utils;
mod workspace;

pub use loader::{
    LoadedProject, load_project, parse_manifest, validate_manifest, validate_service_paths,
    write_manifest_to_disk,
};
pub use schema::{ManifestProxy, ManifestService, ProjectManifest, WorkspaceStrategy};
pub use service::services_from_manifest;
pub use utils::{resolve_service_cwd, slugify, stable_id};
