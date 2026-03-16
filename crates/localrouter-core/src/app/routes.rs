use std::collections::BTreeMap;

use crate::{
    manifest::{slugify, stable_id},
    models::{Project, Route, RouteStatus, ServiceDef, Workspace},
};

use super::RuntimeState;

pub(crate) fn route_host(
    project_name: &str,
    workspace_slug: &str,
    service_route: &str,
    dns_suffix: &str,
) -> String {
    format!(
        "{}.{}.{}{}",
        workspace_slug,
        slugify(service_route),
        slugify(project_name),
        dns_suffix
    )
}

pub(crate) fn short_route_host(
    project_name: &str,
    service_route: &str,
    dns_suffix: &str,
) -> String {
    format!(
        "{}.{}{}",
        slugify(service_route),
        slugify(project_name),
        dns_suffix
    )
}

pub(crate) fn reconcile_routes_locked(inner: &mut RuntimeState) {
    let mut route_ids_by_pattern: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for route in inner.routes.values() {
        route_ids_by_pattern
            .entry(route.pattern.clone())
            .or_default()
            .push(route.id.clone());
    }

    for route_ids in route_ids_by_pattern.values() {
        let mut route_rows = route_ids
            .iter()
            .filter_map(|route_id| {
                let route = inner.routes.get(route_id)?;
                let running = inner.instances.values().any(|instance| {
                    instance.service_id == route.service_id
                        && instance.workspace_id == route.workspace_id
                        && instance.pid > 0
                });
                Some((
                    route.id.clone(),
                    running,
                    route.status.clone(),
                    route.pattern.clone(),
                ))
            })
            .collect::<Vec<_>>();
        route_rows.sort_by(|left, right| left.0.cmp(&right.0));

        let active_winner = if route_rows.len() == 1 {
            route_rows
                .iter()
                .find(|(_, running, _, _)| *running)
                .map(|(route_id, _, _, _)| route_id.clone())
        } else {
            route_rows
                .iter()
                .find(|(_, running, status, _)| *running && *status == RouteStatus::Active)
                .or_else(|| route_rows.iter().find(|(_, running, _, _)| *running))
                .map(|(route_id, _, _, _)| route_id.clone())
        };

        for (route_id, running, _, pattern) in route_rows {
            if let Some(route) = inner.routes.get_mut(&route_id) {
                if route_ids.len() == 1 {
                    route.status = if running {
                        RouteStatus::Active
                    } else {
                        RouteStatus::Stale
                    };
                    route.conflict_reason = None;
                    continue;
                }

                if active_winner.as_deref() == Some(route_id.as_str()) {
                    route.status = RouteStatus::Active;
                    route.conflict_reason = None;
                } else {
                    route.status = RouteStatus::Conflict;
                    route.conflict_reason = Some(if running {
                        format!("host {pattern} is already claimed by another running instance")
                    } else {
                        format!("host {pattern} duplicates another route definition")
                    });
                }
            }
        }
    }
}

pub(crate) fn regenerate_routes_locked(
    inner: &mut RuntimeState,
    dns_suffix: &str,
    proxy_port: u16,
) {
    let projects = inner.projects.values().cloned().collect::<Vec<_>>();
    let workspaces = inner.workspaces.values().cloned().collect::<Vec<_>>();
    let services = inner.services.values().cloned().collect::<Vec<_>>();
    let instances = inner.instances.values().cloned().collect::<Vec<_>>();

    inner.routes.clear();

    for project in &projects {
        let project_workspaces = workspaces
            .iter()
            .filter(|workspace| workspace.project_id == project.id)
            .cloned()
            .collect::<Vec<_>>();
        let include_short_alias = project_workspaces.len() == 1;
        let project_services = services
            .iter()
            .filter(|service| service.project_id == project.id && service.route != "none")
            .cloned()
            .collect::<Vec<_>>();

        for workspace in &project_workspaces {
            for service in &project_services {
                let target = instances
                    .iter()
                    .find(|instance| {
                        instance.project_id == project.id
                            && instance.workspace_id == workspace.id
                            && instance.service_id == service.id
                            && instance.pid > 0
                            && instance.port > 0
                    })
                    .map(|instance| format!("127.0.0.1:{}", instance.port));

                for route in build_project_routes(
                    project,
                    workspace,
                    service,
                    include_short_alias,
                    target.clone(),
                    dns_suffix,
                    proxy_port,
                ) {
                    inner.routes.insert(route.id.clone(), route);
                }
            }
        }
    }

    reconcile_routes_locked(inner);
}

pub(crate) fn build_project_routes(
    project: &Project,
    workspace: &Workspace,
    service: &ServiceDef,
    include_short_alias: bool,
    target: Option<String>,
    dns_suffix: &str,
    proxy_port: u16,
) -> Vec<Route> {
    if project.proxy_disabled {
        return Vec::new();
    }
    let route_specs = [
        Some((
            stable_id(
                "rt",
                &format!("{}:{}:{}:workspace", project.id, workspace.id, service.id),
            ),
            route_host(&project.name, &workspace.slug, &service.route, dns_suffix),
        )),
        include_short_alias.then(|| {
            (
                stable_id(
                    "rt",
                    &format!("{}:{}:{}:short", project.id, workspace.id, service.id),
                ),
                short_route_host(&project.name, &service.route, dns_suffix),
            )
        }),
    ];

    route_specs
        .into_iter()
        .flatten()
        .map(|(id, pattern)| Route {
            id,
            url: route_public_url(&pattern, proxy_port),
            pattern,
            target: target.clone().unwrap_or_else(|| "—".to_string()),
            service_id: service.id.clone(),
            service_name: service.name.clone(),
            workspace_id: workspace.id.clone(),
            workspace_name: workspace.name.clone(),
            project_id: project.id.clone(),
            project_name: project.name.clone(),
            status: RouteStatus::Stale,
            conflict_reason: None,
        })
        .collect()
}

pub(crate) fn route_public_url(pattern: &str, proxy_port: u16) -> String {
    if proxy_port == 80 {
        format!("http://{pattern}")
    } else {
        format!("http://{pattern}:{proxy_port}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{HealthStatus, Instance, Project, now_rfc3339};

    fn sample_project() -> Project {
        Project {
            id: "proj-1".to_string(),
            name: "local-router".to_string(),
            path: "/tmp/local-router".to_string(),
            created_at: now_rfc3339(),
            config_source: "manifest".to_string(),
            proxy_disabled: false,
        }
    }

    fn sample_workspace() -> Workspace {
        Workspace {
            id: "ws-1".to_string(),
            project_id: "proj-1".to_string(),
            name: "main".to_string(),
            branch: "main".to_string(),
            path: "/tmp/local-router".to_string(),
            is_active: true,
            slug: "main".to_string(),
        }
    }

    fn sample_service() -> ServiceDef {
        ServiceDef {
            id: "svc-1".to_string(),
            project_id: "proj-1".to_string(),
            name: "dashboard".to_string(),
            command: "npm run dev -- --port ${PORT}".to_string(),
            protocol: "http".to_string(),
            adapter: "vite".to_string(),
            route: "dashboard".to_string(),
            healthcheck: "http://127.0.0.1:${PORT}".to_string(),
            language: "typescript".to_string(),
            cwd: None,
            env: BTreeMap::new(),
            depends_on: Vec::new(),
            enabled: true,
        }
    }

    #[test]
    fn single_workspace_project_gets_short_alias_route() {
        let routes = build_project_routes(
            &sample_project(),
            &sample_workspace(),
            &sample_service(),
            true,
            None,
            ".localhost",
            9730,
        );
        assert_eq!(routes.len(), 2);
        assert!(
            routes
                .iter()
                .any(|route| route.pattern == "main.dashboard.local-router.localhost")
        );
        assert!(
            routes
                .iter()
                .any(|route| route.pattern == "dashboard.local-router.localhost")
        );
    }

    #[test]
    fn conflicting_routes_mark_only_one_active() {
        let mut inner = RuntimeState::default();
        let mut route_a = build_project_routes(
            &sample_project(),
            &sample_workspace(),
            &sample_service(),
            false,
            Some("127.0.0.1:3000".to_string()),
            ".localhost",
            9730,
        )
        .remove(0);
        route_a.status = RouteStatus::Active;
        let mut route_b = route_a.clone();
        route_b.id = "rt-2".to_string();
        route_b.project_id = "proj-2".to_string();
        route_b.workspace_id = "ws-2".to_string();
        route_b.target = "127.0.0.1:3001".to_string();
        inner.routes.insert(route_a.id.clone(), route_a.clone());
        inner.routes.insert(route_b.id.clone(), route_b.clone());
        inner.instances.insert(
            "inst-1".to_string(),
            Instance {
                id: "inst-1".to_string(),
                service_id: route_a.service_id.clone(),
                service_name: route_a.service_name.clone(),
                workspace_id: route_a.workspace_id.clone(),
                workspace_name: route_a.workspace_name.clone(),
                project_id: route_a.project_id.clone(),
                project_name: route_a.project_name.clone(),
                port: 3000,
                pid: 11,
                status: HealthStatus::Healthy,
                url: String::new(),
                uptime: "1m".to_string(),
                cpu: 0.0,
                memory: 0,
                started_at: None,
                last_exit: None,
                status_reason: None,
            },
        );
        inner.instances.insert(
            "inst-2".to_string(),
            Instance {
                id: "inst-2".to_string(),
                service_id: route_b.service_id.clone(),
                service_name: route_b.service_name.clone(),
                workspace_id: route_b.workspace_id.clone(),
                workspace_name: route_b.workspace_name.clone(),
                project_id: route_b.project_id.clone(),
                project_name: route_b.project_name.clone(),
                port: 3001,
                pid: 12,
                status: HealthStatus::Healthy,
                url: String::new(),
                uptime: "1m".to_string(),
                cpu: 0.0,
                memory: 0,
                started_at: None,
                last_exit: None,
                status_reason: None,
            },
        );

        reconcile_routes_locked(&mut inner);

        let statuses = inner
            .routes
            .values()
            .map(|route| route.status.clone())
            .collect::<Vec<_>>();
        assert_eq!(
            statuses
                .iter()
                .filter(|status| **status == RouteStatus::Active)
                .count(),
            1
        );
        assert_eq!(
            statuses
                .iter()
                .filter(|status| **status == RouteStatus::Conflict)
                .count(),
            1
        );
    }

    #[test]
    fn route_public_url_uses_proxy_port() {
        assert_eq!(
            route_public_url("dashboard.local-router.localhost", 9730),
            "http://dashboard.local-router.localhost:9730"
        );
        assert_eq!(
            route_public_url("dashboard.local-router.localhost", 80),
            "http://dashboard.local-router.localhost"
        );
    }
}
