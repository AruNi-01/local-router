use std::path::Path;

use serde_json::Value;

use super::{
    repo::{PackageManager, RepoLayout},
    shared::{
        CandidateKind, DetectedCandidate, DetectionConfidence, build_http_service,
        build_non_http_service, dependency_haystack, is_library_candidate, normalize_package_name,
        pick_node_script, relative_source_key,
    },
};

pub fn detect_node_candidate(
    project_root: &Path,
    package_json_path: &Path,
    parsed: &Value,
    repo: &RepoLayout,
) -> Option<DetectedCandidate> {
    let scripts = parsed.get("scripts").and_then(Value::as_object)?;
    let (script_name, script_body) = pick_node_script(scripts)?;
    let package_dir = package_json_path.parent().unwrap_or(project_root);
    let (relative, source_key) = relative_source_key(project_root, package_dir);
    let package_name = parsed
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let dir_name = package_dir
        .file_name()
        .and_then(|segment| segment.to_str())
        .unwrap_or("app");
    let service_name = normalize_package_name(&package_name, dir_name);

    if is_workspace_orchestrator(parsed, &script_body, relative.as_deref(), repo) {
        return Some(DetectedCandidate::hidden(
            service_name,
            source_key,
            CandidateKind::WorkspaceRoot,
            DetectionConfidence::Low,
        ));
    }

    let adapter = infer_node_adapter(dir_name, &package_name, &script_body, parsed);
    let language = infer_node_language(parsed, &adapter);
    let lower = format!(
        "{} {} {} {} {}",
        dir_name,
        package_name,
        script_name,
        script_body,
        dependency_haystack(parsed)
    )
    .to_ascii_lowercase();

    if is_library_candidate(relative.as_deref(), &service_name, &lower) {
        return Some(DetectedCandidate::hidden(
            service_name,
            source_key,
            CandidateKind::Library,
            DetectionConfidence::Low,
        ));
    }

    if is_non_http_node_adapter(&adapter) {
        let confidence = if adapter == "worker" {
            DetectionConfidence::Review
        } else {
            DetectionConfidence::High
        };
        return Some(DetectedCandidate::non_http(
            service_name,
            source_key,
            confidence,
            build_non_http_service(
                repo.package_manager.run_script(&script_name),
                relative,
                adapter,
                language,
            ),
        ));
    }

    let confidence = infer_node_confidence(&adapter, relative.as_deref(), &lower);
    if confidence == DetectionConfidence::Low {
        return Some(DetectedCandidate::hidden(
            service_name,
            source_key,
            CandidateKind::Tooling,
            DetectionConfidence::Low,
        ));
    }

    Some(DetectedCandidate::http(
        service_name.clone(),
        source_key,
        confidence,
        build_http_service(
            node_run_command(repo.package_manager, &script_name, &adapter),
            relative,
            adapter,
            service_name,
            language,
        ),
    ))
}

fn infer_node_adapter(
    dir_name: &str,
    package_name: &str,
    dev_script: &str,
    parsed: &Value,
) -> String {
    let lower = format!(
        "{} {} {} {}",
        dir_name,
        package_name,
        dev_script,
        dependency_haystack(parsed)
    )
    .to_ascii_lowercase();
    if lower.contains("tauri") {
        "tauri".to_string()
    } else if lower.contains("electron") {
        "electron".to_string()
    } else if lower.contains("worker")
        || lower.contains("queue")
        || lower.contains("bullmq")
        || lower.contains("consumer")
    {
        "worker".to_string()
    } else if lower.contains("nuxt") {
        "nuxt".to_string()
    } else if lower.contains("next") {
        "nextjs".to_string()
    } else if lower.contains("astro") {
        "astro".to_string()
    } else if lower.contains("remix") {
        "remix".to_string()
    } else if lower.contains("sveltekit") || lower.contains("@sveltejs/kit") {
        "sveltekit".to_string()
    } else if lower.contains("vite") {
        "vite".to_string()
    } else if lower.contains("vue-cli-service") || lower.contains(" vue ") {
        "vue".to_string()
    } else if lower.contains("@nestjs") || lower.contains("nest start") {
        "nest".to_string()
    } else if lower.contains("fastify") {
        "fastify".to_string()
    } else if lower.contains("express") {
        "express".to_string()
    } else if lower.contains("hono") {
        "hono".to_string()
    } else if lower.contains("koa") {
        "koa".to_string()
    } else {
        "generic".to_string()
    }
}

fn infer_node_language(parsed: &Value, adapter: &str) -> String {
    if matches!(
        adapter,
        "nextjs"
            | "nuxt"
            | "astro"
            | "remix"
            | "sveltekit"
            | "vite"
            | "vue"
            | "nest"
            | "fastify"
            | "express"
            | "hono"
            | "koa"
    ) {
        return "typescript".to_string();
    }
    let haystack = dependency_haystack(parsed).to_ascii_lowercase();
    if haystack.contains("typescript")
        || haystack.contains("@types/")
        || parsed.get("type").and_then(Value::as_str) == Some("module")
    {
        "typescript".to_string()
    } else {
        "javascript".to_string()
    }
}

fn is_workspace_orchestrator(
    parsed: &Value,
    dev_script: &str,
    relative: Option<&str>,
    repo: &RepoLayout,
) -> bool {
    if relative.is_some() {
        return false;
    }
    let has_workspaces = parsed.get("workspaces").is_some() || repo.has_node_workspace;
    let lower = dev_script.to_ascii_lowercase();
    has_workspaces
        && (lower.contains("--filter")
            || lower.contains("bun --filter")
            || lower.contains("pnpm -r")
            || lower.contains("turbo")
            || lower.contains("nx ")
            || lower.contains("workspace")
            || lower.contains("run-many"))
}

fn is_non_http_node_adapter(adapter: &str) -> bool {
    matches!(adapter, "tauri" | "electron" | "worker")
}

fn infer_node_confidence(
    adapter: &str,
    relative: Option<&str>,
    lower: &str,
) -> DetectionConfidence {
    if matches!(
        adapter,
        "nextjs"
            | "nuxt"
            | "astro"
            | "remix"
            | "sveltekit"
            | "vite"
            | "vue"
            | "nest"
            | "fastify"
            | "express"
            | "hono"
            | "koa"
    ) {
        return DetectionConfidence::High;
    }
    if let Some(relative) = relative {
        if relative.starts_with("apps/")
            || relative.starts_with("services/")
            || relative.starts_with("apps-web/")
            || lower.contains("listen")
            || lower.contains("server")
        {
            return DetectionConfidence::Review;
        }
    }
    DetectionConfidence::Low
}

fn node_run_command(package_manager: PackageManager, script_name: &str, adapter: &str) -> String {
    let base = package_manager.run_script(script_name);
    match adapter {
        "nextjs" => format!("{base} -- --hostname ${{HOST}} --port ${{PORT}}"),
        "nuxt" | "astro" | "remix" | "sveltekit" | "vite" | "vue" => {
            format!("{base} -- --host ${{HOST}} --port ${{PORT}}")
        }
        _ => base,
    }
}
