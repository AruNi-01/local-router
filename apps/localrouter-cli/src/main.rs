use std::{
    env, fs,
    os::unix::process::CommandExt,
    path::{Path, PathBuf},
    process::Stdio,
    time::Duration,
};

use anyhow::{Context, Result, anyhow};
use clap::{Parser, Subcommand};
use localrouter_core::{
    models::{
        AddProjectRequest, GraphSnapshot, HealthResponse, Instance, Project, ProjectDetail, Route,
    },
    storage::{localrouter_data_dir, pid_file_path},
};
use nix::errno::Errno;
use nix::{
    sys::signal::{Signal, kill},
    unistd::Pid,
};
use reqwest::Client;

#[derive(Parser)]
#[command(name = "localrouter")]
struct Cli {
    #[arg(long, global = true)]
    json: bool,
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Daemon {
        #[command(subcommand)]
        command: DaemonCommand,
    },
    Reset {
        #[command(subcommand)]
        command: ResetCommand,
    },
    Project {
        #[command(subcommand)]
        command: ProjectCommand,
    },
    Ps,
    Up {
        target: Option<String>,
    },
    Down {
        target: Option<String>,
    },
    Restart {
        target: String,
    },
    Logs {
        target: String,
    },
    Routes,
    Open {
        target: String,
    },
    Doctor,
    Graph,
    Dev {
        path: Option<PathBuf>,
        #[arg(long)]
        no_open: bool,
    },
}

#[derive(Subcommand)]
enum DaemonCommand {
    Start,
    Stop,
    Status,
}

#[derive(Subcommand)]
enum ResetCommand {
    All,
    Project { target: Option<String> },
}

#[derive(Subcommand)]
enum ProjectCommand {
    Add { path: Option<PathBuf> },
    List,
    Remove { target: String },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let client = Client::new();

    match cli.command {
        Commands::Daemon { command } => daemon_command(command, cli.json, &client).await,
        Commands::Reset { command } => reset_command(command, cli.json, &client).await,
        Commands::Project { command } => project_command(command, cli.json, &client).await,
        Commands::Ps => {
            ensure_daemon_running(&client, None).await?;
            let instances = fetch_json::<Vec<Instance>>(&client, "/instances").await?;
            print_output(cli.json, &instances, human_instances(&instances));
            Ok(())
        }
        Commands::Up { target } => {
            ensure_daemon_running(&client, None).await?;
            let changed = control_instances(&client, target.as_deref(), "start").await?;
            print_output(cli.json, &changed, human_instances(&changed));
            Ok(())
        }
        Commands::Down { target } => {
            ensure_daemon_running(&client, None).await?;
            let changed = control_instances(&client, target.as_deref(), "stop").await?;
            print_output(cli.json, &changed, human_instances(&changed));
            Ok(())
        }
        Commands::Restart { target } => {
            ensure_daemon_running(&client, None).await?;
            let changed = control_instances(&client, Some(&target), "restart").await?;
            print_output(cli.json, &changed, human_instances(&changed));
            Ok(())
        }
        Commands::Logs { target } => {
            ensure_daemon_running(&client, None).await?;
            let instance = resolve_instances(&client, Some(&target))
                .await?
                .into_iter()
                .next()
                .ok_or_else(|| anyhow!("no matching instance"))?;
            let logs: serde_json::Value =
                fetch_json(&client, &format!("/logs?instance_id={}", instance.id)).await?;
            print_output(cli.json, &logs, serde_json::to_string_pretty(&logs)?);
            Ok(())
        }
        Commands::Routes => {
            ensure_daemon_running(&client, None).await?;
            let routes = fetch_json::<Vec<Route>>(&client, "/routes").await?;
            print_output(cli.json, &routes, human_routes(&routes));
            Ok(())
        }
        Commands::Open { target } => {
            ensure_daemon_running(&client, None).await?;
            let instances = resolve_instances(&client, Some(&target)).await?;
            let url = instances
                .into_iter()
                .find(|instance| !instance.url.is_empty())
                .map(|instance| instance.url)
                .ok_or_else(|| anyhow!("no matching routable instance"))?;
            webbrowser::open(&url)?;
            println!("{url}");
            Ok(())
        }
        Commands::Doctor => {
            ensure_daemon_running(&client, None).await?;
            let health = fetch_json::<HealthResponse>(&client, "/health").await?;
            let instances = fetch_json::<Vec<Instance>>(&client, "/instances").await?;
            let routes = fetch_json::<Vec<Route>>(&client, "/routes").await?;
            let mut report = vec![format!(
                "daemon ok={} api={} proxy={}",
                health.ok, health.api_port, health.proxy_port
            )];
            if let Some(instance) = instances.iter().find(|instance| {
                matches!(
                    instance.status,
                    localrouter_core::models::HealthStatus::Unhealthy
                )
            }) {
                report.push(format!(
                    "unhealthy instance: {}/{}",
                    instance.project_name, instance.service_name
                ));
            }
            if let Some(route) = routes
                .iter()
                .find(|route| route.status == localrouter_core::models::RouteStatus::Conflict)
            {
                report.push(format!("route conflict: {}", route.pattern));
            }
            print_output(
                cli.json,
                &serde_json::json!({ "health": health, "instances": instances, "routes": routes }),
                report.join("\n"),
            );
            Ok(())
        }
        Commands::Graph => {
            ensure_daemon_running(&client, None).await?;
            let graph = fetch_json::<GraphSnapshot>(&client, "/graph").await?;
            print_output(
                cli.json,
                &graph,
                format!("nodes: {}\nedges: {}", graph.nodes.len(), graph.edges.len()),
            );
            Ok(())
        }
        Commands::Dev { path, no_open } => dev_command(&client, cli.json, path, no_open).await,
    }
}

async fn daemon_command(command: DaemonCommand, json: bool, client: &Client) -> Result<()> {
    match command {
        DaemonCommand::Start => {
            if let Ok(health) = fetch_json::<HealthResponse>(client, "/health").await {
                print_output(
                    json,
                    &health,
                    format!("daemon already running on {}", health.api_port),
                );
                return Ok(());
            }
            let cwd = env::current_dir().ok();
            let health = start_daemon(client, cwd.as_deref()).await?;
            print_output(
                json,
                &health,
                format!("daemon started on {}", health.api_port),
            );
            Ok(())
        }
        DaemonCommand::Stop => {
            match stop_daemon_via_pid_file()? {
                StopResult::Stopped => println!("daemon stopped"),
                StopResult::AlreadyStopped => println!("daemon already stopped"),
            }
            Ok(())
        }
        DaemonCommand::Status => {
            match fetch_json::<HealthResponse>(client, "/health").await {
                Ok(health) => {
                    print_output(json, &health, format!("daemon ok on {}", health.api_port));
                }
                Err(_) if json => {
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&serde_json::json!({
                            "running": false
                        }))?
                    );
                }
                Err(_) => {
                    println!("daemon not running");
                }
            }
            Ok(())
        }
    }
}

async fn reset_command(command: ResetCommand, json: bool, client: &Client) -> Result<()> {
    match command {
        ResetCommand::All => {
            let data_dir = localrouter_data_dir();
            let daemon = match stop_daemon_via_pid_file()? {
                StopResult::Stopped => "stopped",
                StopResult::AlreadyStopped => "already_stopped",
            };

            let removed = if data_dir.exists() {
                fs::remove_dir_all(&data_dir)
                    .with_context(|| format!("failed to remove {}", data_dir.display()))?;
                true
            } else {
                false
            };

            let payload = serde_json::json!({
                "scope": "all",
                "daemon": daemon,
                "removed": removed,
                "dataDir": data_dir,
            });
            let human = if removed {
                format!(
                    "cleared localrouter state\n{}\ndaemon={daemon}",
                    data_dir.display()
                )
            } else {
                format!(
                    "localrouter state already clean\n{}\ndaemon={daemon}",
                    data_dir.display()
                )
            };
            print_output(json, &payload, human);
            Ok(())
        }
        ResetCommand::Project { target } => {
            let target = match target {
                Some(target) => target,
                None => env::current_dir()?.to_string_lossy().to_string(),
            };
            ensure_daemon_running(client, None).await?;
            let projects = fetch_json::<Vec<Project>>(client, "/projects").await?;
            let project = projects
                .into_iter()
                .find(|project| {
                    project.id == target || project.name == target || project.path == target
                })
                .ok_or_else(|| anyhow!("project not found"))?;
            delete_empty(client, &format!("/projects/{}", project.id)).await?;
            let payload = serde_json::json!({
                "scope": "project",
                "projectId": project.id,
                "name": project.name,
                "path": project.path,
            });
            let human = format!("removed project {}\n{}", project.name, project.path);
            print_output(json, &payload, human);
            Ok(())
        }
    }
}

async fn project_command(command: ProjectCommand, json: bool, client: &Client) -> Result<()> {
    match command {
        ProjectCommand::Add { path } => {
            let path = path.unwrap_or(env::current_dir()?);
            ensure_daemon_running(client, Some(&path)).await?;
            let detail = ensure_project_registered(client, &path).await?;
            print_output(json, &detail, human_project_detail(&detail));
            Ok(())
        }
        ProjectCommand::List => {
            ensure_daemon_running(client, None).await?;
            let projects = fetch_json::<Vec<Project>>(client, "/projects").await?;
            print_output(json, &projects, human_projects(&projects));
            Ok(())
        }
        ProjectCommand::Remove { target } => {
            ensure_daemon_running(client, None).await?;
            let projects = fetch_json::<Vec<Project>>(client, "/projects").await?;
            let project = projects
                .into_iter()
                .find(|project| {
                    project.id == target || project.name == target || project.path == target
                })
                .ok_or_else(|| anyhow!("project not found"))?;
            delete_empty(client, &format!("/projects/{}", project.id)).await?;
            println!("removed {}", project.name);
            Ok(())
        }
    }
}

async fn dev_command(
    client: &Client,
    json: bool,
    path: Option<PathBuf>,
    no_open: bool,
) -> Result<()> {
    let path = path.unwrap_or(env::current_dir()?);
    ensure_daemon_running(client, Some(&path)).await?;
    let detail = ensure_project_registered(client, &path).await?;
    let instances = control_instances(client, Some(&detail.project.id), "start").await?;
    let dashboard_url = dashboard_url();

    if !no_open {
        let _ = webbrowser::open(&dashboard_url);
    }

    let payload = serde_json::json!({
        "project": detail.project,
        "instances": instances,
        "dashboardUrl": dashboard_url,
    });
    let human = format!(
        "ready {}  {}\ninstances={}\ndashboard={}",
        detail.project.name,
        detail.project.path,
        instances.len(),
        dashboard_url
    );
    print_output(json, &payload, human);
    Ok(())
}

async fn ensure_daemon_running(client: &Client, bootstrap_dir: Option<&Path>) -> Result<()> {
    if fetch_json::<HealthResponse>(client, "/health")
        .await
        .is_ok()
    {
        return Ok(());
    }
    eprintln!("localrouterd is not running; starting it automatically");
    start_daemon(client, bootstrap_dir).await?;
    Ok(())
}

async fn start_daemon(client: &Client, bootstrap_dir: Option<&Path>) -> Result<HealthResponse> {
    let daemon_bin = ensure_daemon_binary()?;
    let mut command = std::process::Command::new(&daemon_bin);
    command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    if let Some(dir) = bootstrap_dir {
        command.current_dir(dir);
    }
    unsafe {
        command.pre_exec(|| {
            nix::unistd::setsid().map_err(|error| std::io::Error::other(error.to_string()))?;
            Ok(())
        });
    }
    command
        .spawn()
        .with_context(|| format!("failed to spawn {}", daemon_bin.display()))?;

    for _ in 0..24 {
        tokio::time::sleep(Duration::from_millis(250)).await;
        if let Ok(health) = fetch_json::<HealthResponse>(client, "/health").await {
            return Ok(health);
        }
    }

    Err(anyhow!("daemon failed to become healthy"))
}

enum StopResult {
    Stopped,
    AlreadyStopped,
}

fn stop_daemon_via_pid_file() -> Result<StopResult> {
    let pid = match fs::read_to_string(pid_file_path()) {
        Ok(pid) => pid,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(StopResult::AlreadyStopped);
        }
        Err(error) => return Err(error).context("failed to read daemon pid file"),
    };
    match kill(Pid::from_raw(pid.trim().parse::<i32>()?), Signal::SIGTERM) {
        Ok(()) | Err(Errno::ESRCH) => {}
        Err(error) => return Err(error.into()),
    }
    let _ = fs::remove_file(pid_file_path());
    Ok(StopResult::Stopped)
}

fn ensure_daemon_binary() -> Result<PathBuf> {
    let current = env::current_exe()?;
    let daemon_bin = current.with_file_name("localrouterd");
    if daemon_bin.exists() {
        return Ok(daemon_bin);
    }

    if let Some(workspace_root) = current
        .ancestors()
        .find(|ancestor| ancestor.join("Cargo.toml").exists())
    {
        let status = std::process::Command::new("cargo")
            .arg("build")
            .arg("-p")
            .arg("localrouterd")
            .current_dir(workspace_root)
            .status()
            .context("failed to build localrouterd automatically")?;
        if !status.success() {
            return Err(anyhow!("cargo build -p localrouterd failed"));
        }
        if daemon_bin.exists() {
            return Ok(daemon_bin);
        }
    }

    Err(anyhow!(
        "localrouterd binary not found next to the CLI; build the workspace first"
    ))
}

async fn ensure_project_registered(client: &Client, path: &Path) -> Result<ProjectDetail> {
    let canonical = path
        .canonicalize()
        .with_context(|| format!("failed to resolve project path {}", path.display()))?;
    let canonical_str = canonical.to_string_lossy().to_string();
    let projects = fetch_json::<Vec<Project>>(client, "/projects").await?;

    if let Some(project) = projects
        .into_iter()
        .find(|project| project.path == canonical_str)
    {
        return post_empty::<ProjectDetail>(client, &format!("/projects/{}/rescan", project.id))
            .await;
    }

    post_json::<_, ProjectDetail>(
        client,
        "/projects",
        &AddProjectRequest {
            path: canonical_str,
        },
    )
    .await
}

fn dashboard_url() -> String {
    let api_base =
        env::var("LOCALROUTER_API").unwrap_or_else(|_| "http://127.0.0.1:9731/v1".to_string());
    if let Some(stripped) = api_base.strip_suffix("/v1") {
        stripped.to_string()
    } else {
        api_base
    }
}

async fn control_instances(
    client: &Client,
    target: Option<&str>,
    action: &str,
) -> Result<Vec<Instance>> {
    let instances = resolve_instances(client, target).await?;
    let mut changed = Vec::new();
    for instance in instances {
        let next =
            post_empty::<Instance>(client, &format!("/instances/{}/{}", instance.id, action))
                .await?;
        changed.push(next);
    }
    Ok(changed)
}

async fn resolve_instances(client: &Client, target: Option<&str>) -> Result<Vec<Instance>> {
    let instances = fetch_json::<Vec<Instance>>(client, "/instances").await?;
    let Some(target) = target else {
        return Ok(instances);
    };
    let matched = instances
        .into_iter()
        .filter(|instance| {
            instance.id == target
                || instance.service_id == target
                || instance.service_name == target
                || instance.project_id == target
                || instance.project_name == target
                || instance.workspace_id == target
                || instance.workspace_name == target
                || instance.url.contains(target)
        })
        .collect::<Vec<_>>();
    if matched.is_empty() {
        return Err(anyhow!("no instances matched target {target}"));
    }
    Ok(matched)
}

async fn fetch_json<T: serde::de::DeserializeOwned>(client: &Client, path: &str) -> Result<T> {
    let response = client.get(api_url(path)).send().await?.error_for_status()?;
    Ok(response.json::<T>().await?)
}

async fn post_empty<T: serde::de::DeserializeOwned>(client: &Client, path: &str) -> Result<T> {
    let response = client
        .post(api_url(path))
        .send()
        .await?
        .error_for_status()?;
    Ok(response.json::<T>().await?)
}

async fn post_json<T: serde::Serialize, U: serde::de::DeserializeOwned>(
    client: &Client,
    path: &str,
    body: &T,
) -> Result<U> {
    let response = client
        .post(api_url(path))
        .json(body)
        .send()
        .await?
        .error_for_status()?;
    Ok(response.json::<U>().await?)
}

async fn delete_empty(client: &Client, path: &str) -> Result<()> {
    client
        .delete(api_url(path))
        .send()
        .await?
        .error_for_status()?;
    Ok(())
}

fn api_url(path: &str) -> String {
    let base =
        env::var("LOCALROUTER_API").unwrap_or_else(|_| "http://127.0.0.1:9731/v1".to_string());
    format!("{base}{path}")
}

fn print_output<T: serde::Serialize>(json_mode: bool, value: &T, human: String) {
    if json_mode {
        println!(
            "{}",
            serde_json::to_string_pretty(value).unwrap_or_else(|_| "{}".to_string())
        );
    } else {
        println!("{human}");
    }
}

fn human_projects(projects: &[Project]) -> String {
    if projects.is_empty() {
        return "no projects".to_string();
    }
    projects
        .iter()
        .map(|project| format!("{}  {}", project.name, project.path))
        .collect::<Vec<_>>()
        .join("\n")
}

fn human_project_detail(detail: &ProjectDetail) -> String {
    format!(
        "imported {}  {}\nworkspaces={} services={} routes={}",
        detail.project.name,
        detail.project.path,
        detail.workspaces.len(),
        detail.services.len(),
        detail.routes.len()
    )
}

fn human_instances(instances: &[Instance]) -> String {
    if instances.is_empty() {
        return "no instances".to_string();
    }
    instances
        .iter()
        .map(|instance| {
            format!(
                "{}/{} [{}] status={:?} pid={} port={} {}",
                instance.project_name,
                instance.service_name,
                instance.workspace_name,
                instance.status,
                instance.pid,
                instance.port,
                instance.url
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn human_routes(routes: &[Route]) -> String {
    if routes.is_empty() {
        return "no routes".to_string();
    }
    routes
        .iter()
        .map(|route| format!("{} -> {} [{:?}]", route.pattern, route.target, route.status))
        .collect::<Vec<_>>()
        .join("\n")
}
