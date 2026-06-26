use std::{
    collections::{BTreeMap, HashSet},
    net::{Ipv4Addr, SocketAddrV4, TcpListener},
    process::Stdio,
    sync::Arc,
    time::Duration,
};

use anyhow::{Context, Result, anyhow};
use nix::{
    sys::signal::{Signal, kill},
    unistd::Pid,
};
use serde_json::json;
use tokio::{
    process::Command,
    sync::watch,
    time::{Instant, sleep},
};

use crate::{
    manifest::{normalize_service_env_name, resolve_service_cwd},
    models::{
        HealthStatus, Instance, LogEntry, LogLevel, Project, RouteStatus, ServiceDef, Workspace,
        now_rfc3339, uptime_string,
    },
};

use super::{
    AppState,
    routes::{reconcile_routes_locked, route_host},
};

const LOOPBACK_HOST: &str = "127.0.0.1";
impl AppState {
    pub async fn start_instance(&self, instance_id: &str) -> Result<Instance> {
        let dependency_ready_timeout =
            Duration::from_secs(self.config.read().await.dependency_ready_timeout);
        let start_order = self.build_instance_start_order(instance_id).await?;
        let mut requested_instance = None;

        for ordered_instance_id in start_order {
            let started = self.start_single_instance(&ordered_instance_id).await?;
            if ordered_instance_id == instance_id {
                requested_instance = Some(started);
            } else {
                self.wait_for_dependency_ready(&ordered_instance_id, dependency_ready_timeout)
                    .await?;
            }
        }

        requested_instance.ok_or_else(|| anyhow!("instance not found"))
    }

    async fn build_instance_start_order(&self, instance_id: &str) -> Result<Vec<String>> {
        let inner = self.inner.read().await;
        let mut visited = HashSet::new();
        let mut stack = Vec::new();
        let mut order = Vec::new();

        fn visit(
            instance_id: &str,
            inner: &super::RuntimeState,
            visited: &mut HashSet<String>,
            stack: &mut Vec<String>,
            order: &mut Vec<String>,
        ) -> Result<()> {
            if visited.contains(instance_id) {
                return Ok(());
            }
            if let Some(cycle_start) = stack.iter().position(|id| id == instance_id) {
                let mut cycle = stack[cycle_start..].to_vec();
                cycle.push(instance_id.to_string());
                return Err(anyhow!(
                    "service dependency cycle detected: {}",
                    cycle.join(" -> ")
                ));
            }

            let instance = inner
                .instances
                .get(instance_id)
                .ok_or_else(|| anyhow!("instance not found"))?;
            let service = inner
                .services
                .get(&instance.service_id)
                .ok_or_else(|| anyhow!("service not found"))?;

            stack.push(instance_id.to_string());
            for dependency_name in &service.depends_on {
                let dependency_service = inner
                    .services
                    .values()
                    .find(|candidate| {
                        candidate.project_id == service.project_id
                            && candidate.name == *dependency_name
                    })
                    .ok_or_else(|| {
                        anyhow!(
                            "service '{}' depends on missing service '{}'",
                            service.name,
                            dependency_name
                        )
                    })?;
                let dependency_instance = inner
                    .instances
                    .values()
                    .find(|candidate| {
                        candidate.workspace_id == instance.workspace_id
                            && candidate.service_id == dependency_service.id
                    })
                    .ok_or_else(|| {
                        anyhow!(
                            "service '{}' depends on '{}' but no instance exists in workspace '{}'",
                            service.name,
                            dependency_name,
                            instance.workspace_name
                        )
                    })?;
                visit(&dependency_instance.id, inner, visited, stack, order)?;
            }
            stack.pop();

            visited.insert(instance_id.to_string());
            order.push(instance_id.to_string());
            Ok(())
        }

        visit(instance_id, &inner, &mut visited, &mut stack, &mut order)?;
        Ok(order)
    }

    async fn wait_for_dependency_ready(&self, instance_id: &str, timeout: Duration) -> Result<()> {
        let deadline = Instant::now() + timeout;
        loop {
            let snapshot = {
                let inner = self.inner.read().await;
                inner.instances.get(instance_id).cloned()
            }
            .ok_or_else(|| anyhow!("dependency instance not found"))?;

            if snapshot.status == HealthStatus::Healthy {
                return Ok(());
            }
            if matches!(
                snapshot.status,
                HealthStatus::Unhealthy | HealthStatus::Stopped
            ) {
                return Err(anyhow!(
                    "dependency '{}' reached terminal status {:?} before becoming healthy{}",
                    snapshot.service_name,
                    snapshot.status,
                    snapshot
                        .status_reason
                        .as_deref()
                        .map(|reason| format!(" ({reason})"))
                        .unwrap_or_default()
                ));
            }
            if Instant::now() >= deadline {
                return Err(anyhow!(
                    "dependency '{}' did not become healthy within {}s; last status: {:?}{}",
                    snapshot.service_name,
                    timeout.as_secs(),
                    snapshot.status,
                    snapshot
                        .status_reason
                        .as_deref()
                        .map(|reason| format!(" ({reason})"))
                        .unwrap_or_default()
                ));
            }

            sleep(Duration::from_millis(250)).await;
        }
    }

    async fn start_single_instance(&self, instance_id: &str) -> Result<Instance> {
        let start_lock = self.instance_start_lock(instance_id).await;
        let _start_guard = start_lock.lock().await;

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
        let runtime_env = self
            .build_runtime_env(&service, &workspace, port, &public_url)
            .await?;
        let args = build_runtime_argv(&service, port, &public_url)?;
        let (program, argv) = args
            .split_first()
            .ok_or_else(|| anyhow!("command is empty"))?;

        let cwd = resolve_service_cwd(&project.path, service.cwd.as_deref());
        let mut process = Command::new(program);
        process
            .args(argv)
            .current_dir(&cwd)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .envs(runtime_env);

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

    async fn build_runtime_env(
        &self,
        service: &ServiceDef,
        workspace: &Workspace,
        port: u16,
        public_url: &str,
    ) -> Result<BTreeMap<String, String>> {
        let mut env = BTreeMap::from([
            ("PORT".to_string(), port.to_string()),
            ("HOST".to_string(), LOOPBACK_HOST.to_string()),
            ("PUBLIC_URL".to_string(), public_url.to_string()),
            ("LOCALROUTER_PORT".to_string(), port.to_string()),
            ("LOCALROUTER_HOST".to_string(), LOOPBACK_HOST.to_string()),
            ("LOCALROUTER_PUBLIC_URL".to_string(), public_url.to_string()),
            ("FLASK_RUN_HOST".to_string(), LOOPBACK_HOST.to_string()),
            ("FLASK_RUN_PORT".to_string(), port.to_string()),
        ]);

        env.extend(self.dependency_service_env(service, workspace).await?);
        let template_context = env.clone();
        for (key, value) in &service.env {
            env.insert(key.clone(), render_env_templates(value, &template_context));
        }

        Ok(env)
    }

    async fn dependency_service_env(
        &self,
        service: &ServiceDef,
        workspace: &Workspace,
    ) -> Result<BTreeMap<String, String>> {
        let inner = self.inner.read().await;
        let mut env = BTreeMap::new();
        let mut dependency_services = Vec::new();
        let mut dependency_by_env_name = BTreeMap::<String, String>::new();

        for dependency_name in &service.depends_on {
            let dependency_service = inner
                .services
                .values()
                .find(|candidate| {
                    candidate.project_id == service.project_id && candidate.name == *dependency_name
                })
                .ok_or_else(|| {
                    anyhow!(
                        "service '{}' depends on missing service '{}'",
                        service.name,
                        dependency_name
                    )
                })?;
            let env_name = normalize_service_env_name(&dependency_service.name);
            if env_name.is_empty() {
                return Err(anyhow!(
                    "service '{}': dependency '{}' normalizes to an empty LOCALROUTER_SERVICE suffix",
                    service.name,
                    dependency_service.name
                ));
            }
            if let Some(existing) = dependency_by_env_name.get(&env_name) {
                if existing != &dependency_service.name {
                    return Err(anyhow!(
                        "service '{}': dependencies '{}' and '{}' both normalize to LOCALROUTER_SERVICE_{}",
                        service.name,
                        existing,
                        dependency_service.name,
                        env_name
                    ));
                }
                continue;
            }
            dependency_by_env_name.insert(env_name.clone(), dependency_service.name.clone());
            dependency_services.push((
                dependency_name.clone(),
                dependency_service.clone(),
                env_name,
            ));
        }

        for (dependency_name, dependency_service, env_name) in dependency_services {
            let dependency_instance = inner
                .instances
                .values()
                .find(|candidate| {
                    candidate.workspace_id == workspace.id
                        && candidate.service_id == dependency_service.id
                })
                .ok_or_else(|| {
                    anyhow!(
                        "service '{}' depends on '{}' but no instance exists in workspace '{}'",
                        service.name,
                        dependency_name,
                        workspace.name
                    )
                })?;
            if dependency_instance.port > 0 {
                env.insert(
                    format!("LOCALROUTER_SERVICE_{env_name}_PORT"),
                    dependency_instance.port.to_string(),
                );
            }

            let route_url = (!dependency_instance.url.is_empty())
                .then(|| dependency_instance.url.clone())
                .or_else(|| {
                    inner
                        .routes
                        .values()
                        .find(|route| {
                            route.workspace_id == workspace.id
                                && route.service_id == dependency_service.id
                                && route.status == RouteStatus::Active
                        })
                        .map(|route| route.url.clone())
                        .filter(|url| !url.is_empty())
                })
                .or_else(|| {
                    (dependency_instance.port > 0)
                        .then(|| format!("http://127.0.0.1:{}", dependency_instance.port))
                });
            if let Some(route_url) = route_url {
                env.insert(format!("LOCALROUTER_SERVICE_{env_name}_URL"), route_url);
            }
        }

        Ok(env)
    }

    async fn instance_start_lock(&self, instance_id: &str) -> Arc<tokio::sync::Mutex<()>> {
        let mut start_locks = self.start_locks.lock().await;
        start_locks
            .entry(instance_id.to_string())
            .or_insert_with(|| Arc::new(tokio::sync::Mutex::new(())))
            .clone()
    }

    pub async fn stop_instance(&self, instance_id: &str) -> Result<Instance> {
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
                instance.status_reason = Some("Stopped by user.".to_string());
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

pub(crate) fn render_env_templates(input: &str, context: &BTreeMap<String, String>) -> String {
    let mut rendered = input.to_string();
    for (key, value) in context {
        rendered = rendered.replace(&format!("${{{key}}}"), value);
    }
    rendered
}

fn instance_source(project: &Project, service: &ServiceDef, workspace: &Workspace) -> String {
    if workspace.name == "main" {
        format!("{}/{}", project.name, service.name)
    } else {
        format!("{}/{}[{}]", project.name, service.name, workspace.name)
    }
}

#[cfg(test)]
mod tests {
    use std::{collections::BTreeMap, time::Duration};

    use super::{
        LOOPBACK_HOST, ServiceDef, build_runtime_argv, find_free_loopback_port, is_port_available,
        render_env_templates,
    };
    use crate::{
        AppState,
        manifest::normalize_service_env_name,
        models::{HealthStatus, Instance, Project, Route, RouteStatus, Workspace, now_rfc3339},
        storage::PersistedState,
    };

    fn sample_service(command: &str, adapter: &str) -> ServiceDef {
        ServiceDef {
            id: "svc_1".to_string(),
            project_id: "proj_1".to_string(),
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

    fn sample_project() -> Project {
        Project {
            id: "proj_1".to_string(),
            name: "demo".to_string(),
            path: "/tmp/demo".to_string(),
            created_at: now_rfc3339(),
            config_source: "manifest".to_string(),
            proxy_disabled: false,
        }
    }

    fn sample_workspace() -> Workspace {
        Workspace {
            id: "ws_1".to_string(),
            project_id: "proj_1".to_string(),
            name: "feature-login".to_string(),
            branch: "feature/login".to_string(),
            path: "/tmp/demo-worktree".to_string(),
            is_active: true,
            slug: "feature-login".to_string(),
        }
    }

    fn service_with_deps(id: &str, name: &str, route: &str, depends_on: Vec<&str>) -> ServiceDef {
        ServiceDef {
            id: id.to_string(),
            project_id: "proj_1".to_string(),
            name: name.to_string(),
            command: "node server.js".to_string(),
            protocol: "http".to_string(),
            adapter: "express".to_string(),
            route: route.to_string(),
            healthcheck: "http://127.0.0.1:${PORT}".to_string(),
            language: "typescript".to_string(),
            cwd: None,
            env: Default::default(),
            depends_on: depends_on.into_iter().map(str::to_string).collect(),
            enabled: true,
        }
    }

    fn stopped_instance(id: &str, service: &ServiceDef, workspace: &Workspace) -> Instance {
        Instance {
            id: id.to_string(),
            service_id: service.id.clone(),
            service_name: service.name.clone(),
            workspace_id: workspace.id.clone(),
            workspace_name: workspace.name.clone(),
            project_id: "proj_1".to_string(),
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
            status_reason: None,
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

    #[test]
    fn normalizes_service_names_for_env_keys() {
        assert_eq!(normalize_service_env_name("api-server"), "API_SERVER");
        assert_eq!(normalize_service_env_name("api.server"), "API_SERVER");
        assert_eq!(normalize_service_env_name("--api server--"), "API_SERVER");
    }

    #[test]
    fn renders_env_templates_from_context() {
        let context = BTreeMap::from([
            (
                "LOCALROUTER_SERVICE_API_URL".to_string(),
                "http://feature-login.api.demo.localhost:9730".to_string(),
            ),
            ("PORT".to_string(), "4123".to_string()),
        ]);

        assert_eq!(
            render_env_templates("${LOCALROUTER_SERVICE_API_URL}/v1/${PORT}", &context),
            "http://feature-login.api.demo.localhost:9730/v1/4123"
        );
    }

    #[tokio::test]
    async fn dependency_start_order_places_dependencies_first() {
        let project = sample_project();
        let workspace = sample_workspace();
        let api = service_with_deps("svc_api", "api", "api", Vec::new());
        let web = service_with_deps("svc_web", "web", "web", vec!["api"]);
        let api_instance = stopped_instance("inst_api", &api, &workspace);
        let web_instance = stopped_instance("inst_web", &web, &workspace);
        let app = AppState::for_tests(PersistedState {
            projects: vec![project],
            workspaces: vec![workspace],
            services: vec![api, web],
            instances: vec![api_instance, web_instance],
            ..PersistedState::default()
        })
        .await
        .unwrap();

        let order = app.build_instance_start_order("inst_web").await.unwrap();

        assert_eq!(order, vec!["inst_api", "inst_web"]);
    }

    #[tokio::test]
    async fn dependency_env_injects_peer_url_and_renders_service_env() {
        let project = sample_project();
        let workspace = sample_workspace();
        let api = service_with_deps("svc_api", "api-server", "api", Vec::new());
        let mut web = service_with_deps("svc_web", "web", "web", vec!["api-server"]);
        web.env.insert(
            "VITE_API_BASE_URL".to_string(),
            "${LOCALROUTER_SERVICE_API_SERVER_URL}".to_string(),
        );
        let mut api_instance = stopped_instance("inst_api", &api, &workspace);
        api_instance.port = 4100;
        api_instance.pid = 42;
        api_instance.status = HealthStatus::Healthy;
        api_instance.url = "http://feature-login.api.demo.localhost:9730".to_string();
        let web_instance = stopped_instance("inst_web", &web, &workspace);
        let route = Route {
            id: "rt_api".to_string(),
            pattern: "feature-login.api.demo.localhost".to_string(),
            url: api_instance.url.clone(),
            target: "127.0.0.1:4100".to_string(),
            service_id: api.id.clone(),
            service_name: api.name.clone(),
            workspace_id: workspace.id.clone(),
            workspace_name: workspace.name.clone(),
            project_id: project.id.clone(),
            project_name: project.name.clone(),
            status: RouteStatus::Active,
            conflict_reason: None,
        };
        let app = AppState::for_tests(PersistedState {
            projects: vec![project],
            workspaces: vec![workspace.clone()],
            services: vec![api, web.clone()],
            instances: vec![api_instance, web_instance],
            routes: vec![route],
            ..PersistedState::default()
        })
        .await
        .unwrap();

        let env = app
            .build_runtime_env(
                &web,
                &workspace,
                4200,
                "http://feature-login.web.demo.localhost:9730",
            )
            .await
            .unwrap();

        assert_eq!(
            env.get("LOCALROUTER_SERVICE_API_SERVER_URL")
                .map(String::as_str),
            Some("http://feature-login.api.demo.localhost:9730")
        );
        assert_eq!(
            env.get("LOCALROUTER_SERVICE_API_SERVER_PORT")
                .map(String::as_str),
            Some("4100")
        );
        assert_eq!(
            env.get("VITE_API_BASE_URL").map(String::as_str),
            Some("http://feature-login.api.demo.localhost:9730")
        );
    }

    #[tokio::test]
    async fn dependency_env_rejects_normalized_name_collisions() {
        let project = sample_project();
        let workspace = sample_workspace();
        let api_dash = service_with_deps("svc_api_dash", "api-server", "api-dash", Vec::new());
        let api_dot = service_with_deps("svc_api_dot", "api.server", "api-dot", Vec::new());
        let web = service_with_deps("svc_web", "web", "web", vec!["api-server", "api.server"]);
        let app = AppState::for_tests(PersistedState {
            projects: vec![project],
            workspaces: vec![workspace.clone()],
            services: vec![api_dash.clone(), api_dot.clone(), web.clone()],
            instances: vec![
                stopped_instance("inst_api_dash", &api_dash, &workspace),
                stopped_instance("inst_api_dot", &api_dot, &workspace),
                stopped_instance("inst_web", &web, &workspace),
            ],
            ..PersistedState::default()
        })
        .await
        .unwrap();

        let error = app
            .build_runtime_env(
                &web,
                &workspace,
                4200,
                "http://feature-login.web.demo.localhost:9730",
            )
            .await
            .unwrap_err()
            .to_string();

        assert!(
            error.contains("both normalize to LOCALROUTER_SERVICE_API_SERVER"),
            "expected env name collision error, got: {error}"
        );
    }

    #[tokio::test]
    async fn dependency_env_falls_back_to_loopback_url_for_non_routed_dependencies() {
        let project = sample_project();
        let workspace = sample_workspace();
        let worker = service_with_deps("svc_worker", "worker", "none", Vec::new());
        let web = service_with_deps("svc_web", "web", "web", vec!["worker"]);
        let mut worker_instance = stopped_instance("inst_worker", &worker, &workspace);
        worker_instance.port = 4321;
        worker_instance.pid = 43;
        worker_instance.status = HealthStatus::Healthy;
        let web_instance = stopped_instance("inst_web", &web, &workspace);
        let app = AppState::for_tests(PersistedState {
            projects: vec![project],
            workspaces: vec![workspace.clone()],
            services: vec![worker, web.clone()],
            instances: vec![worker_instance, web_instance],
            ..PersistedState::default()
        })
        .await
        .unwrap();

        let env = app
            .build_runtime_env(
                &web,
                &workspace,
                4200,
                "http://feature-login.web.demo.localhost:9730",
            )
            .await
            .unwrap();

        assert_eq!(
            env.get("LOCALROUTER_SERVICE_WORKER_URL")
                .map(String::as_str),
            Some("http://127.0.0.1:4321")
        );
    }

    #[tokio::test]
    async fn dependency_env_rejects_empty_normalized_names() {
        let project = sample_project();
        let workspace = sample_workspace();
        let service = service_with_deps("svc_empty", "---", "dash-service", Vec::new());
        let web = service_with_deps("svc_web", "web", "web", vec!["---"]);
        let app = AppState::for_tests(PersistedState {
            projects: vec![project],
            workspaces: vec![workspace.clone()],
            services: vec![service.clone(), web.clone()],
            instances: vec![
                stopped_instance("inst_empty", &service, &workspace),
                stopped_instance("inst_web", &web, &workspace),
            ],
            ..PersistedState::default()
        })
        .await
        .unwrap();

        let error = app
            .build_runtime_env(
                &web,
                &workspace,
                4200,
                "http://feature-login.web.demo.localhost:9730",
            )
            .await
            .unwrap_err()
            .to_string();

        assert!(
            error.contains("empty LOCALROUTER_SERVICE suffix"),
            "expected empty env suffix error, got: {error}"
        );
    }

    #[tokio::test]
    async fn dependency_wait_fails_fast_for_terminal_status() {
        let project = sample_project();
        let workspace = sample_workspace();
        let api = service_with_deps("svc_api", "api", "api", Vec::new());
        let mut api_instance = stopped_instance("inst_api", &api, &workspace);
        api_instance.status = HealthStatus::Unhealthy;
        api_instance.status_reason = Some("Process exited with code 1.".to_string());
        let app = AppState::for_tests(PersistedState {
            projects: vec![project],
            workspaces: vec![workspace],
            services: vec![api],
            instances: vec![api_instance],
            ..PersistedState::default()
        })
        .await
        .unwrap();

        let error = app
            .wait_for_dependency_ready("inst_api", Duration::from_secs(30))
            .await
            .unwrap_err()
            .to_string();

        assert!(
            error.contains("terminal status Unhealthy")
                && error.contains("Process exited with code 1."),
            "expected terminal status error with reason, got: {error}"
        );
    }
}
