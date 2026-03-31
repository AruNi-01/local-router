use std::{
    net::{Ipv4Addr, SocketAddrV4, TcpListener},
    process::Stdio,
    time::Duration,
};

use anyhow::{Context, Result, anyhow};
use nix::{
    sys::signal::{Signal, kill},
    unistd::Pid,
};
use serde_json::json;
use tokio::{process::Command, sync::watch, time::sleep};

use crate::{
    manifest::resolve_service_cwd,
    models::{
        HealthStatus, Instance, LogEntry, LogLevel, Project, ServiceDef, Workspace, now_rfc3339,
        uptime_string,
    },
};

use super::{
    AppState,
    routes::{reconcile_routes_locked, route_host},
};

const LOOPBACK_HOST: &str = "127.0.0.1";

impl AppState {
    pub async fn start_instance(&self, instance_id: &str) -> Result<Instance> {
        let config = self.config.read().await.clone();
        let (service, workspace, project, existing_instance) = {
            let inner = self.inner.read().await;
            let instance = inner
                .instances
                .get(instance_id)
                .cloned()
                .ok_or_else(|| anyhow!("instance not found"))?;
            if !matches!(
                instance.status,
                HealthStatus::Stopped | HealthStatus::Unknown
            ) && instance.pid > 0
            {
                return Ok(instance);
            }
            let service = inner
                .services
                .get(&instance.service_id)
                .cloned()
                .ok_or_else(|| anyhow!("service not found"))?;
            let workspace = inner
                .workspaces
                .get(&instance.workspace_id)
                .cloned()
                .ok_or_else(|| anyhow!("workspace not found"))?;
            let project = inner
                .projects
                .get(&instance.project_id)
                .cloned()
                .ok_or_else(|| anyhow!("project not found"))?;
            (service, workspace, project, instance)
        };

        if !service.enabled {
            let disabled_instance = {
                let mut inner = self.inner.write().await;
                if let Some(instance) = inner.instances.get_mut(instance_id) {
                    instance.status = HealthStatus::Stopped;
                    instance.pid = 0;
                    instance.port = 0;
                    instance.url.clear();
                    instance.cpu = 0.0;
                    instance.memory = 0;
                    instance.started_at = None;
                    instance.uptime = "—".to_string();
                    instance.status_reason = Some(disabled_instance_reason(&service));
                    instance.clone()
                } else {
                    existing_instance.clone()
                }
            };
            self.emit(
                "instance_skipped",
                json!({ "instanceId": instance_id, "reason": disabled_instance_reason(&service) }),
            )
            .await;
            self.persist().await?;
            return Ok(disabled_instance);
        }

        let port = self.allocate_port(instance_id).await;
        let host = route_host(
            &project.name,
            &workspace.slug,
            &service.route,
            &config.dns_suffix,
        );
        let public_url = if service.route == "none" {
            String::new()
        } else {
            format!("http://{host}:{}", config.proxy_port)
        };
        let args = build_runtime_argv(&service, port, &public_url)?;
        let (program, argv) = args
            .split_first()
            .ok_or_else(|| anyhow!("command is empty"))?;

        let cwd = resolve_service_cwd(&workspace.path, service.cwd.as_deref());
        let mut process = Command::new(program);
        process
            .args(argv)
            .current_dir(&cwd)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .env("PORT", port.to_string())
            .env("HOST", LOOPBACK_HOST)
            .env("PUBLIC_URL", &public_url)
            .env("LOCALROUTER_PORT", port.to_string())
            .env("LOCALROUTER_HOST", LOOPBACK_HOST)
            .env("LOCALROUTER_PUBLIC_URL", &public_url)
            .env("FLASK_RUN_HOST", LOOPBACK_HOST)
            .env("FLASK_RUN_PORT", port.to_string());
        for (key, value) in &service.env {
            process.env(key, value);
        }

        let mut child = process
            .spawn()
            .with_context(|| format!("failed to spawn {}", service.name))?;
        let pid = child.id().unwrap_or_default();
        let stdout = child.stdout.take();
        let stderr = child.stderr.take();

        if let Some(stdout) = stdout {
            self.spawn_log_task(
                stdout,
                instance_id.to_string(),
                instance_source(&project, &service, &workspace),
                LogLevel::Info,
            );
        }
        if let Some(stderr) = stderr {
            self.spawn_log_task(
                stderr,
                instance_id.to_string(),
                instance_source(&project, &service, &workspace),
                LogLevel::Error,
            );
        }

        let (stop_tx, stop_rx) = watch::channel(false);
        self.health_tasks
            .lock()
            .await
            .insert(instance_id.to_string(), stop_tx);

        {
            let mut inner = self.inner.write().await;
            if let Some(instance) = inner.instances.get_mut(instance_id) {
                instance.port = port;
                instance.pid = pid;
                instance.url = public_url.clone();
                instance.status = HealthStatus::Starting;
                instance.started_at = Some(now_rfc3339());
                instance.uptime = uptime_string(instance.started_at.as_deref());
                instance.last_exit = None;
                instance.status_reason =
                    Some(if service.protocol == "http" && service.route != "none" {
                        format!("Waiting for healthcheck at http://127.0.0.1:{port}.")
                    } else {
                        "Process is starting.".to_string()
                    });
            }

            for route in inner.routes.values_mut().filter(|route| {
                route.service_id == service.id && route.workspace_id == workspace.id
            }) {
                route.target = format!("127.0.0.1:{port}");
            }
            reconcile_routes_locked(&mut inner);
        }
        self.emit(
            "instance_starting",
            json!({ "instanceId": instance_id, "pid": pid, "port": port }),
        )
        .await;
        self.emit(
            "route_registered",
            json!({ "instanceId": instance_id, "host": host }),
        )
        .await;
        self.persist().await?;

        let app = self.clone();
        let service_for_wait = service.clone();
        let workspace_for_wait = workspace.clone();
        let project_for_wait = project.clone();
        let instance_id_wait = instance_id.to_string();
        tokio::spawn(async move {
            let code = child.wait().await.ok().and_then(|status| status.code());
            app.on_process_exit(&instance_id_wait, pid, code).await;
            let _ = app
                .append_log(LogEntry {
                    timestamp: now_rfc3339(),
                    level: if code.unwrap_or_default() == 0 {
                        LogLevel::Info
                    } else {
                        LogLevel::Error
                    },
                    source: instance_source(
                        &project_for_wait,
                        &service_for_wait,
                        &workspace_for_wait,
                    ),
                    message: format!("process exited with code {}", code.unwrap_or(-1)),
                    instance_id: instance_id_wait.clone(),
                })
                .await;
        });

        self.spawn_health_task(instance_id.to_string(), stop_rx, service, port)
            .await;
        Ok(self
            .inner
            .read()
            .await
            .instances
            .get(instance_id)
            .cloned()
            .unwrap_or(existing_instance))
    }

    pub async fn stop_instance(&self, instance_id: &str) -> Result<Instance> {
        self.stop_instance_inner(instance_id, "Stopped by user.")
            .await
    }

    pub(crate) async fn stop_instance_inner(
        &self,
        instance_id: &str,
        reason: &str,
    ) -> Result<Instance> {
        let pid = self
            .inner
            .read()
            .await
            .instances
            .get(instance_id)
            .map(|instance| instance.pid)
            .ok_or_else(|| anyhow!("instance not found"))?;
        if pid == 0 {
            return self
                .inner
                .read()
                .await
                .instances
                .get(instance_id)
                .cloned()
                .ok_or_else(|| anyhow!("instance not found"));
        }

        if let Some(cancel) = self.health_tasks.lock().await.remove(instance_id) {
            let _ = cancel.send(true);
        }

        kill(Pid::from_raw(pid as i32), Signal::SIGTERM).ok();
        sleep(Duration::from_millis(250)).await;
        {
            let mut inner = self.inner.write().await;
            let route_service_id = inner
                .instances
                .get(instance_id)
                .map(|instance| instance.service_id.clone());
            let route_workspace_id = inner
                .instances
                .get(instance_id)
                .map(|instance| instance.workspace_id.clone());
            if let Some(instance) = inner.instances.get_mut(instance_id) {
                instance.status = HealthStatus::Stopped;
                instance.pid = 0;
                instance.cpu = 0.0;
                instance.memory = 0;
                instance.started_at = None;
                instance.uptime = "—".to_string();
                instance.status_reason = Some(reason.to_string());
            }
            if let (Some(route_service_id), Some(route_workspace_id)) =
                (route_service_id, route_workspace_id)
            {
                for route in inner.routes.values_mut().filter(|route| {
                    route.service_id == route_service_id && route.workspace_id == route_workspace_id
                }) {
                    route.target = "—".to_string();
                }
            }
            reconcile_routes_locked(&mut inner);
        }
        self.emit("instance_stopped", json!({ "instanceId": instance_id }))
            .await;
        self.persist().await?;
        self.inner
            .read()
            .await
            .instances
            .get(instance_id)
            .cloned()
            .ok_or_else(|| anyhow!("instance not found"))
    }

    pub async fn restart_instance(&self, instance_id: &str) -> Result<Instance> {
        let _ = self.stop_instance(instance_id).await;
        sleep(Duration::from_millis(350)).await;
        self.start_instance(instance_id).await
    }

    pub async fn start_service(&self, service_id: &str) -> Result<Vec<Instance>> {
        let instance_ids = self
            .inner
            .read()
            .await
            .instances
            .values()
            .filter(|instance| instance.service_id == service_id)
            .map(|instance| instance.id.clone())
            .collect::<Vec<_>>();
        let mut started = Vec::new();
        for instance_id in instance_ids {
            started.push(self.start_instance(&instance_id).await?);
        }
        Ok(started)
    }

    pub async fn stop_service(&self, service_id: &str) -> Result<Vec<Instance>> {
        let instance_ids = self
            .inner
            .read()
            .await
            .instances
            .values()
            .filter(|instance| instance.service_id == service_id)
            .map(|instance| instance.id.clone())
            .collect::<Vec<_>>();
        let mut stopped = Vec::new();
        for instance_id in instance_ids {
            stopped.push(self.stop_instance(&instance_id).await?);
        }
        Ok(stopped)
    }

    pub(crate) async fn on_process_exit(&self, instance_id: &str, pid: u32, code: Option<i32>) {
        if let Some(cancel) = self.health_tasks.lock().await.remove(instance_id) {
            let _ = cancel.send(true);
        }
        {
            let mut inner = self.inner.write().await;
            let Some(instance) = inner.instances.get_mut(instance_id) else {
                return;
            };
            if instance.pid != pid {
                return;
            }
            let route_service_id = instance.service_id.clone();
            let route_workspace_id = instance.workspace_id.clone();
            instance.pid = 0;
            instance.cpu = 0.0;
            instance.memory = 0;
            instance.status = if code.unwrap_or_default() == 0 {
                HealthStatus::Stopped
            } else {
                HealthStatus::Unhealthy
            };
            instance.last_exit = code;
            instance.started_at = None;
            instance.uptime = "—".to_string();
            instance.status_reason = Some(match code {
                Some(0) => "Process exited normally.".to_string(),
                Some(code) => format!("Process exited with code {code}."),
                None => "Process exited unexpectedly.".to_string(),
            });

            for route in inner.routes.values_mut().filter(|route| {
                route.service_id == route_service_id && route.workspace_id == route_workspace_id
            }) {
                route.target = "—".to_string();
            }
            reconcile_routes_locked(&mut inner);
        }
        self.emit(
            "instance_failed",
            json!({ "instanceId": instance_id, "exitCode": code }),
        )
        .await;
        let _ = self.persist().await;
    }

    async fn allocate_port(&self, instance_id: &str) -> u16 {
        let inner = self.inner.read().await;
        if let Some(existing) = inner.instances.get(instance_id) {
            if existing.port != 0 && is_port_available(existing.port) {
                return existing.port;
            }
        }
        let used = inner
            .instances
            .values()
            .filter(|instance| {
                instance.id != instance_id && instance.port != 0 && instance.pid != 0
            })
            .map(|instance| instance.port)
            .collect::<Vec<_>>();
        find_free_loopback_port(&used).unwrap_or(3000)
    }
}

fn build_runtime_argv(service: &ServiceDef, port: u16, public_url: &str) -> Result<Vec<String>> {
    let command = render_command(&service.command, port, public_url);
    let mut args = shlex::split(&command).ok_or_else(|| anyhow!("failed to parse command"))?;
    normalize_runtime_args(service, &mut args, port);
    Ok(args)
}

fn normalize_runtime_args(service: &ServiceDef, args: &mut Vec<String>, port: u16) {
    if service.protocol != "http" || service.route == "none" || args.is_empty() {
        return;
    }

    let runtime_flags = adapter_runtime_flags(&service.adapter, port);
    if runtime_flags.is_empty() {
        return;
    }

    match detect_script_runner(args) {
        Some(ScriptRunner::Npm | ScriptRunner::Pnpm | ScriptRunner::Bun) => {
            rewrite_script_runner_args(args, &service.adapter, &runtime_flags);
        }
        Some(ScriptRunner::Yarn) => {
            rewrite_yarn_script_args(args, &service.adapter, &runtime_flags);
        }
        None => {
            rewrite_direct_command_args(args, &service.adapter, &runtime_flags);
        }
    }
}

fn adapter_runtime_flags(adapter: &str, port: u16) -> Vec<String> {
    match adapter {
        "nextjs" => vec![
            "--hostname".to_string(),
            LOOPBACK_HOST.to_string(),
            "--port".to_string(),
            port.to_string(),
        ],
        "nuxt" | "astro" | "remix" | "sveltekit" | "vite" | "vue" | "angular" => vec![
            "--host".to_string(),
            LOOPBACK_HOST.to_string(),
            "--port".to_string(),
            port.to_string(),
            "--strictPort".to_string(),
        ],
        "fastapi" | "starlette" => vec![
            "--host".to_string(),
            LOOPBACK_HOST.to_string(),
            "--port".to_string(),
            port.to_string(),
        ],
        "django" => vec![format!("{LOOPBACK_HOST}:{port}")],
        // Backend Node.js frameworks rely on PORT env var injected by the
        // runtime (line ~89).  No CLI flags exist for these adapters.
        "nest" | "fastify" | "express" | "hono" | "koa" => Vec::new(),
        // Spring Boot / Quarkus use ${PORT} template in the command string,
        // which is already substituted by render_command().
        "spring-boot" | "quarkus" => Vec::new(),
        _ => Vec::new(),
    }
}

#[derive(Clone, Copy)]
enum ScriptRunner {
    Npm,
    Pnpm,
    Bun,
    Yarn,
}

fn detect_script_runner(args: &[String]) -> Option<ScriptRunner> {
    match args {
        [program, run, _, ..] if program == "npm" && run == "run" => Some(ScriptRunner::Npm),
        [program, run, _, ..] if program == "pnpm" && run == "run" => Some(ScriptRunner::Pnpm),
        [program, run, _, ..] if program == "bun" && run == "run" => Some(ScriptRunner::Bun),
        [program, _, ..] if program == "yarn" => Some(ScriptRunner::Yarn),
        _ => None,
    }
}

fn rewrite_script_runner_args(args: &mut Vec<String>, adapter: &str, runtime_flags: &[String]) {
    let mut forwarded = if let Some(index) = args.iter().position(|arg| arg == "--") {
        args.split_off(index + 1)
    } else {
        args.push("--".to_string());
        Vec::new()
    };
    strip_conflicting_runtime_flags(&mut forwarded, adapter);
    args.extend(forwarded);
    args.extend(runtime_flags.iter().cloned());
}

fn rewrite_yarn_script_args(args: &mut Vec<String>, adapter: &str, runtime_flags: &[String]) {
    let split_index = args.len().min(2);
    let mut forwarded = args.split_off(split_index);
    strip_conflicting_runtime_flags(&mut forwarded, adapter);
    args.extend(forwarded);
    args.extend(runtime_flags.iter().cloned());
}

fn rewrite_direct_command_args(args: &mut Vec<String>, adapter: &str, runtime_flags: &[String]) {
    let mut command_args = args.split_off(1);
    strip_conflicting_runtime_flags(&mut command_args, adapter);
    args.extend(command_args);
    args.extend(runtime_flags.iter().cloned());
}

fn strip_conflicting_runtime_flags(args: &mut Vec<String>, adapter: &str) {
    if args.is_empty() {
        return;
    }

    let mut filtered = Vec::with_capacity(args.len());
    let mut index = 0;
    while index < args.len() {
        let arg = &args[index];
        if takes_flag_value(adapter, arg) {
            index += 2;
            continue;
        }
        if is_inline_flag(adapter, arg) || is_boolean_flag(adapter, arg) {
            index += 1;
            continue;
        }
        if adapter == "django" && is_django_bind_arg(arg) {
            index += 1;
            continue;
        }
        filtered.push(arg.clone());
        index += 1;
    }
    *args = filtered;
}

fn takes_flag_value(adapter: &str, arg: &str) -> bool {
    match adapter {
        "nextjs" => matches!(arg, "--hostname" | "--host" | "--port" | "-p" | "-H"),
        "nuxt" | "astro" | "remix" | "sveltekit" | "vite" | "vue" | "angular" | "fastapi"
        | "starlette" => {
            matches!(arg, "--hostname" | "--host" | "--port" | "-p" | "-H")
        }
        _ => false,
    }
}

fn is_inline_flag(adapter: &str, arg: &str) -> bool {
    match adapter {
        "nextjs" | "nuxt" | "astro" | "remix" | "sveltekit" | "vite" | "vue" | "angular"
        | "fastapi" | "starlette" => {
            arg.starts_with("--host=")
                || arg.starts_with("--hostname=")
                || arg.starts_with("--port=")
        }
        _ => false,
    }
}

fn is_boolean_flag(adapter: &str, arg: &str) -> bool {
    matches!(
        adapter,
        "nuxt" | "astro" | "remix" | "sveltekit" | "vite" | "vue" | "angular"
    ) && matches!(arg, "--strictPort" | "--strict-port")
}

fn is_django_bind_arg(arg: &str) -> bool {
    if arg.chars().all(|ch| ch.is_ascii_digit()) {
        return true;
    }
    let Some((host, port)) = arg.rsplit_once(':') else {
        return false;
    };
    !host.is_empty() && port.chars().all(|ch| ch.is_ascii_digit())
}

fn is_port_available(port: u16) -> bool {
    TcpListener::bind(SocketAddrV4::new(Ipv4Addr::LOCALHOST, port)).is_ok()
}

fn find_free_loopback_port(used: &[u16]) -> Option<u16> {
    for _ in 0..16 {
        let listener = TcpListener::bind(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0)).ok()?;
        let port = listener.local_addr().ok()?.port();
        if !used.contains(&port) {
            drop(listener);
            return Some(port);
        }
    }

    (3000..60000).find(|candidate| !used.contains(candidate) && is_port_available(*candidate))
}

pub(crate) fn render_command(input: &str, port: u16, public_url: &str) -> String {
    input
        .replace("${PORT}", &port.to_string())
        .replace("${HOST}", LOOPBACK_HOST)
        .replace("${PUBLIC_URL}", public_url)
}

fn instance_source(project: &Project, service: &ServiceDef, workspace: &Workspace) -> String {
    if workspace.name == "main" {
        format!("{}/{}", project.name, service.name)
    } else {
        format!("{}/{}[{}]", project.name, service.name, workspace.name)
    }
}

fn disabled_instance_reason(service: &ServiceDef) -> String {
    format!(
        "Disabled in manifest; enable '{}' before starting it.",
        service.name
    )
}

#[cfg(test)]
mod tests {
    use std::{collections::BTreeMap, fs, time::Duration};

    use super::{LOOPBACK_HOST, build_runtime_argv, find_free_loopback_port, is_port_available};
    use crate::{
        AppState,
        models::{HealthStatus, Instance, Project, ServiceDef, Workspace, now_rfc3339},
        storage::PersistedState,
    };
    use tempfile::tempdir;
    use tokio::time::sleep;

    fn sample_service(command: &str, adapter: &str) -> ServiceDef {
        ServiceDef {
            id: "svc_1".to_string(),
            project_id: "proj_1".to_string(),
            workspace_id: Some("ws-1".to_string()),
            name: "web".to_string(),
            command: command.to_string(),
            protocol: "http".to_string(),
            adapter: adapter.to_string(),
            route: "web".to_string(),
            healthcheck: String::new(),
            language: "typescript".to_string(),
            cwd: None,
            env: Default::default(),
            depends_on: Vec::new(),
            enabled: true,
        }
    }

    #[test]
    fn rewrites_vite_script_runner_flags() {
        let args = build_runtime_argv(
            &sample_service("npm run dev -- --port 3000 --host 0.0.0.0", "vite"),
            4123,
            "http://web.localhost:9730",
        )
        .unwrap();

        assert_eq!(
            args,
            vec![
                "npm",
                "run",
                "dev",
                "--",
                "--host",
                LOOPBACK_HOST,
                "--port",
                "4123",
                "--strictPort",
            ]
        );
    }

    #[test]
    fn rewrites_direct_next_command_flags() {
        let args = build_runtime_argv(
            &sample_service("next dev --port 3000", "nextjs"),
            5123,
            "http://web.localhost:9730",
        )
        .unwrap();

        assert_eq!(
            args,
            vec!["next", "dev", "--hostname", LOOPBACK_HOST, "--port", "5123",]
        );
    }

    #[test]
    fn rewrites_django_bind_address() {
        let args = build_runtime_argv(
            &sample_service("python manage.py runserver 0.0.0.0:8000", "django"),
            6123,
            "http://api.localhost:9730",
        )
        .unwrap();

        assert_eq!(
            args,
            vec!["python", "manage.py", "runserver", "127.0.0.1:6123"]
        );
    }

    #[test]
    fn rewrites_angular_serve_flags() {
        let args = build_runtime_argv(
            &sample_service("npm run dev -- --port 4200", "angular"),
            5555,
            "http://app.localhost:9730",
        )
        .unwrap();

        assert_eq!(
            args,
            vec![
                "npm",
                "run",
                "dev",
                "--",
                "--host",
                LOOPBACK_HOST,
                "--port",
                "5555",
                "--strictPort",
            ]
        );
    }

    #[test]
    fn finds_truly_free_loopback_port() {
        let port = find_free_loopback_port(&[]).unwrap();
        assert_ne!(port, 0);
        assert!(is_port_available(port));
    }

    #[tokio::test]
    async fn disabled_services_cannot_start() {
        let service = ServiceDef {
            enabled: false,
            ..sample_service("echo hello", "generic")
        };
        let instance = Instance {
            id: "inst-1".to_string(),
            service_id: service.id.clone(),
            service_name: service.name.clone(),
            workspace_id: "ws-1".to_string(),
            workspace_name: "main".to_string(),
            project_id: "proj-1".to_string(),
            project_name: "demo".to_string(),
            port: 0,
            pid: 0,
            status: HealthStatus::Stopped,
            url: String::new(),
            uptime: "—".to_string(),
            cpu: 0.0,
            memory: 0,
            started_at: None,
            last_exit: None,
            status_reason: Some("Instance is registered but not running.".to_string()),
        };
        let app = AppState::for_tests(PersistedState {
            projects: vec![Project {
                id: "proj-1".to_string(),
                name: "demo".to_string(),
                path: "/tmp/demo".to_string(),
                created_at: now_rfc3339(),
                config_source: "manifest".to_string(),
                proxy_disabled: false,
            }],
            workspaces: vec![Workspace {
                id: "ws-1".to_string(),
                project_id: "proj-1".to_string(),
                name: "main".to_string(),
                branch: "main".to_string(),
                path: "/tmp/demo".to_string(),
                is_active: true,
                slug: "main".to_string(),
            }],
            services: vec![service],
            instances: vec![instance],
            ..PersistedState::default()
        })
        .await
        .unwrap();

        let instance = app.start_instance("inst-1").await.unwrap();
        assert_eq!(instance.status, HealthStatus::Stopped);
        assert_eq!(instance.pid, 0);

        let instances = app.instances().await;
        assert!(
            instances[0]
                .status_reason
                .as_deref()
                .unwrap_or_default()
                .contains("Disabled in manifest")
        );
    }

    #[tokio::test]
    async fn instance_start_uses_workspace_path_for_relative_service_cwd() {
        let temp = tempdir().unwrap();
        let project_root = temp.path().join("project-main");
        let workspace_root = temp.path().join("feature-worktree");
        fs::create_dir_all(project_root.join("svc")).unwrap();
        fs::create_dir_all(workspace_root.join("svc")).unwrap();

        let service = ServiceDef {
            cwd: Some("svc".to_string()),
            command: "sh -c 'pwd > cwd.txt; sleep 1'".to_string(),
            protocol: "none".to_string(),
            route: "none".to_string(),
            env: BTreeMap::new(),
            depends_on: Vec::new(),
            enabled: true,
            ..sample_service("echo hello", "generic")
        };
        let instance = Instance {
            id: "inst-1".to_string(),
            service_id: service.id.clone(),
            service_name: service.name.clone(),
            workspace_id: "ws-1".to_string(),
            workspace_name: "feature".to_string(),
            project_id: "proj-1".to_string(),
            project_name: "demo".to_string(),
            port: 0,
            pid: 0,
            status: HealthStatus::Stopped,
            url: String::new(),
            uptime: "—".to_string(),
            cpu: 0.0,
            memory: 0,
            started_at: None,
            last_exit: None,
            status_reason: Some("Instance is registered but not running.".to_string()),
        };
        let app = AppState::for_tests(PersistedState {
            projects: vec![Project {
                id: "proj-1".to_string(),
                name: "demo".to_string(),
                path: project_root.to_string_lossy().to_string(),
                created_at: now_rfc3339(),
                config_source: "manifest".to_string(),
                proxy_disabled: false,
            }],
            workspaces: vec![Workspace {
                id: "ws-1".to_string(),
                project_id: "proj-1".to_string(),
                name: "feature".to_string(),
                branch: "feature".to_string(),
                path: workspace_root.to_string_lossy().to_string(),
                is_active: true,
                slug: "feature".to_string(),
            }],
            services: vec![service],
            instances: vec![instance],
            ..PersistedState::default()
        })
        .await
        .unwrap();

        let _ = app.start_instance("inst-1").await.unwrap();
        sleep(Duration::from_millis(250)).await;

        let captured = fs::read_to_string(workspace_root.join("svc/cwd.txt")).unwrap();
        let expected = workspace_root.join("svc").canonicalize().unwrap();
        assert_eq!(captured.trim(), expected.to_string_lossy().as_ref());
    }
}
