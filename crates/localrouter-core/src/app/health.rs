use anyhow::Result;
use reqwest::StatusCode;
use serde_json::json;
use sysinfo::{ProcessesToUpdate, System};
use tokio::{
    sync::watch,
    time::{Duration, sleep},
};

use crate::models::{HealthStatus, ServiceDef, uptime_string};

use super::{AppState, RuntimeState};

impl AppState {
    pub(crate) async fn spawn_health_task(
        &self,
        instance_id: String,
        mut stop_rx: watch::Receiver<bool>,
        service: ServiceDef,
        port: u16,
    ) {
        let app = self.clone();
        tokio::spawn(async move {
            if service.healthcheck.is_empty() || service.protocol == "none" {
                sleep(Duration::from_secs(1)).await;
                if !*stop_rx.borrow() {
                    let _ = app
                        .set_instance_status(&instance_id, HealthStatus::Healthy, None)
                        .await;
                }
                return;
            }

            let healthcheck_url = super::runtime::render_command(&service.healthcheck, port, "");
            loop {
                let interval = healthcheck_poll_interval(healthcheck_interval_seconds(&app).await);
                tokio::select! {
                    _ = stop_rx.changed() => {
                        if *stop_rx.borrow() {
                            break;
                        }
                    }
                    _ = sleep(interval) => {
                        match evaluate_healthcheck(&app, &healthcheck_url).await {
                            HealthcheckResult::Healthy => {
                                let _ = app
                                    .set_instance_status(&instance_id, HealthStatus::Healthy, None)
                                    .await;
                            }
                            HealthcheckResult::Unhealthy(reason) => {
                                let _ = app
                                    .set_instance_status(
                                        &instance_id,
                                        HealthStatus::Unhealthy,
                                        Some(reason),
                                    )
                                    .await;
                            }
                        }
                    }
                }
            }
        });
    }

    pub(crate) async fn set_instance_status(
        &self,
        instance_id: &str,
        status: HealthStatus,
        reason: Option<String>,
    ) -> Result<()> {
        {
            let mut inner = self.inner.write().await;
            if let Some(instance) = inner.instances.get_mut(instance_id) {
                if instance.pid == 0 && !matches!(status, HealthStatus::Stopped) {
                    return Ok(());
                }
                instance.status = status.clone();
                instance.uptime = uptime_string(instance.started_at.as_deref());
                instance.status_reason = reason.clone();
            }
        }
        self.emit(
            "health_changed",
            json!({ "instanceId": instance_id, "status": status, "reason": reason }),
        )
        .await;
        self.persist().await?;
        Ok(())
    }

    pub(crate) async fn refresh_metrics(&self) {
        let mut system = System::new_all();
        system.refresh_processes(ProcessesToUpdate::All, true);
        let mut inner = self.inner.write().await;
        for instance in inner.instances.values_mut() {
            if instance.pid == 0 {
                instance.cpu = 0.0;
                instance.memory = 0;
                instance.uptime = uptime_string(instance.started_at.as_deref());
                continue;
            }
            if let Some(process) = system.process(sysinfo::Pid::from_u32(instance.pid)) {
                instance.cpu = process.cpu_usage();
                instance.memory = process.memory() / 1024;
                instance.uptime = uptime_string(instance.started_at.as_deref());
            }
        }
    }
}

pub(crate) fn project_status(inner: &RuntimeState, project_id: &str) -> HealthStatus {
    let statuses = inner
        .instances
        .values()
        .filter(|instance| instance.project_id == project_id)
        .map(|instance| instance.status.clone())
        .collect::<Vec<_>>();
    summarize_status(statuses)
}

pub(crate) fn workspace_status(inner: &RuntimeState, workspace_id: &str) -> HealthStatus {
    let statuses = inner
        .instances
        .values()
        .filter(|instance| instance.workspace_id == workspace_id)
        .map(|instance| instance.status.clone())
        .collect::<Vec<_>>();
    summarize_status(statuses)
}

fn summarize_status(statuses: Vec<HealthStatus>) -> HealthStatus {
    if statuses
        .iter()
        .any(|status| status == &HealthStatus::Unhealthy)
    {
        HealthStatus::Unhealthy
    } else if statuses
        .iter()
        .any(|status| status == &HealthStatus::Starting)
    {
        HealthStatus::Starting
    } else if statuses
        .iter()
        .all(|status| status == &HealthStatus::Stopped)
    {
        HealthStatus::Stopped
    } else if statuses
        .iter()
        .any(|status| status == &HealthStatus::Healthy)
    {
        HealthStatus::Healthy
    } else {
        HealthStatus::Unknown
    }
}

fn healthcheck_response_is_ready(status: StatusCode) -> bool {
    !status.is_server_error()
}

enum HealthcheckResult {
    Healthy,
    Unhealthy(String),
}

async fn evaluate_healthcheck(app: &AppState, healthcheck_url: &str) -> HealthcheckResult {
    match app.client.get(healthcheck_url).send().await {
        Ok(response) if healthcheck_response_is_ready(response.status()) => {
            HealthcheckResult::Healthy
        }
        Ok(response) => HealthcheckResult::Unhealthy(format!(
            "Healthcheck {healthcheck_url} returned {} {}.",
            response.status().as_u16(),
            response
                .status()
                .canonical_reason()
                .unwrap_or("unknown status"),
        )),
        Err(error) => {
            HealthcheckResult::Unhealthy(format!("Healthcheck {healthcheck_url} failed: {error}."))
        }
    }
}

pub(crate) async fn healthcheck_interval_seconds(app: &AppState) -> u64 {
    app.config.read().await.healthcheck_interval
}

fn healthcheck_poll_interval(interval_secs: u64) -> Duration {
    Duration::from_secs(interval_secs.max(1))
}

#[cfg(test)]
mod tests {
    use super::{
        HealthcheckResult, evaluate_healthcheck, healthcheck_poll_interval,
        healthcheck_response_is_ready,
    };
    use crate::{AppState, storage::PersistedState};
    use axum::{Router, routing::get};
    use reqwest::StatusCode;
    use tokio::net::TcpListener;

    #[test]
    fn accepts_redirect_and_client_error_as_ready() {
        assert!(healthcheck_response_is_ready(StatusCode::OK));
        assert!(healthcheck_response_is_ready(
            StatusCode::TEMPORARY_REDIRECT
        ));
        assert!(healthcheck_response_is_ready(StatusCode::UNAUTHORIZED));
        assert!(healthcheck_response_is_ready(StatusCode::NOT_FOUND));
    }

    #[test]
    fn rejects_server_error_as_not_ready() {
        assert!(!healthcheck_response_is_ready(StatusCode::BAD_GATEWAY));
        assert!(!healthcheck_response_is_ready(
            StatusCode::INTERNAL_SERVER_ERROR
        ));
    }

    #[tokio::test]
    async fn healthcheck_failure_carries_reason() {
        let upstream = Router::new().route("/", get(|| async { StatusCode::BAD_GATEWAY }));
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            let _ = axum::serve(listener, upstream).await;
        });
        let app = AppState::for_tests(PersistedState::default())
            .await
            .unwrap();

        let result =
            evaluate_healthcheck(&app, &format!("http://127.0.0.1:{}/", addr.port())).await;
        assert!(matches!(result, HealthcheckResult::Unhealthy(reason) if reason.contains("502")));
    }

    #[test]
    fn healthcheck_interval_clamps_to_at_least_one_second() {
        assert_eq!(
            healthcheck_poll_interval(0),
            std::time::Duration::from_secs(1)
        );
        assert_eq!(
            healthcheck_poll_interval(7),
            std::time::Duration::from_secs(7)
        );
    }
}
