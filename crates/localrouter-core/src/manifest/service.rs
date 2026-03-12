use anyhow::{Result, anyhow};

use crate::models::ServiceDef;

use super::{
    schema::{ManifestService, ProjectManifest},
    utils::stable_id,
};

pub fn services_from_manifest(
    project_id: &str,
    manifest: &ProjectManifest,
) -> Result<Vec<ServiceDef>> {
    manifest
        .services
        .iter()
        .map(|(name, svc)| service_from_manifest(project_id, name, svc))
        .collect()
}

pub fn infer_adapter(name: &str, command: &str) -> String {
    let haystack = format!("{name} {command}").to_ascii_lowercase();
    if haystack.contains("next ") || haystack.contains("nextjs") {
        "nextjs".to_string()
    } else if haystack.contains("nuxt") {
        "nuxt".to_string()
    } else if haystack.contains("astro") {
        "astro".to_string()
    } else if haystack.contains("remix") {
        "remix".to_string()
    } else if haystack.contains("sveltekit") || haystack.contains("svelte-kit") {
        "sveltekit".to_string()
    } else if haystack.contains("vite") {
        "vite".to_string()
    } else if haystack.contains("vue-cli-service") {
        "vue".to_string()
    } else if haystack.contains("nest") {
        "nest".to_string()
    } else if haystack.contains("fastify") {
        "fastify".to_string()
    } else if haystack.contains("express") {
        "express".to_string()
    } else if haystack.contains("hono") {
        "hono".to_string()
    } else if haystack.contains("koa") {
        "koa".to_string()
    } else if haystack.contains("django") {
        "django".to_string()
    } else if haystack.contains("fastapi") || haystack.contains("uvicorn") {
        "fastapi".to_string()
    } else if haystack.contains("flask") {
        "flask".to_string()
    } else if haystack.contains("starlette") {
        "starlette".to_string()
    } else if haystack.contains("go run") {
        "go-http".to_string()
    } else if haystack.contains("cargo") {
        "cargo-bin".to_string()
    } else if haystack.contains("tauri") {
        "tauri".to_string()
    } else if haystack.contains("electron") {
        "electron".to_string()
    } else if haystack.contains("worker") {
        "worker".to_string()
    } else {
        "generic".to_string()
    }
}

pub fn infer_language(adapter: &str) -> &'static str {
    match adapter {
        "nextjs" | "nuxt" | "astro" | "remix" | "sveltekit" | "vite" | "vue" | "nest"
        | "fastify" | "express" | "hono" | "koa" | "tauri" | "electron" => "typescript",
        "uvicorn" | "django" | "fastapi" | "flask" | "starlette" => "python",
        "cargo-bin" => "rust",
        "go-http" => "go",
        _ => "generic",
    }
}

fn service_from_manifest(
    project_id: &str,
    name: &str,
    svc: &ManifestService,
) -> Result<ServiceDef> {
    if svc.command.trim().is_empty() {
        return Err(anyhow!("service {name} is missing command"));
    }

    let adapter = svc
        .adapter
        .clone()
        .unwrap_or_else(|| infer_adapter(name, svc.command.as_str()));
    let protocol = svc.protocol.clone().unwrap_or_else(|| {
        if adapter == "worker" || svc.route.as_deref() == Some("none") {
            "none".to_string()
        } else {
            "http".to_string()
        }
    });
    let route = svc.route.clone().unwrap_or_else(|| name.to_string());
    let healthcheck = svc.healthcheck.clone().unwrap_or_else(|| {
        if protocol == "http" && route != "none" {
            "http://127.0.0.1:${PORT}".to_string()
        } else {
            String::new()
        }
    });

    Ok(ServiceDef {
        id: stable_id("svc", &format!("{project_id}:{name}")),
        project_id: project_id.to_string(),
        name: name.to_string(),
        command: svc.command.clone(),
        protocol,
        adapter: adapter.clone(),
        route,
        healthcheck,
        language: svc
            .language
            .clone()
            .unwrap_or_else(|| infer_language(&adapter).to_string()),
        cwd: svc.cwd.clone(),
        env: svc.env.clone(),
        depends_on: svc.depends_on.clone(),
        enabled: !svc.disabled.unwrap_or(false),
    })
}
