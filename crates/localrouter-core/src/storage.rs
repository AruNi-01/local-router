use std::{
    collections::{BTreeMap, VecDeque},
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use dirs::data_local_dir;
use rusqlite::{Connection, params};

use crate::models::{DaemonConfig, Instance, LogEntry, Project, Route, ServiceDef, Workspace};

#[derive(Debug, Clone)]
pub struct PersistedState {
    pub config: DaemonConfig,
    pub manifests: BTreeMap<String, String>,
    pub projects: Vec<Project>,
    pub workspaces: Vec<Workspace>,
    pub services: Vec<ServiceDef>,
    pub instances: Vec<Instance>,
    pub routes: Vec<Route>,
    pub logs: VecDeque<LogEntry>,
}

impl Default for PersistedState {
    fn default() -> Self {
        Self {
            config: DaemonConfig::default(),
            manifests: BTreeMap::new(),
            projects: Vec::new(),
            workspaces: Vec::new(),
            services: Vec::new(),
            instances: Vec::new(),
            routes: Vec::new(),
            logs: VecDeque::new(),
        }
    }
}

#[derive(Clone)]
pub struct Storage {
    path: PathBuf,
}

pub fn localrouter_data_dir() -> PathBuf {
    data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("localrouter")
}

pub fn pid_file_path() -> PathBuf {
    localrouter_data_dir().join("localrouterd.pid")
}

impl Storage {
    pub fn open_default() -> Result<Self> {
        let base = localrouter_data_dir();
        fs::create_dir_all(&base).context("failed to create localrouter data directory")?;
        let path = base.join("state.sqlite3");
        Self::open_at(path)
    }

    pub fn open_at(path: PathBuf) -> Result<Self> {
        let storage = Self { path };
        storage.init()?;
        Ok(storage)
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    fn init(&self) -> Result<()> {
        let connection = Connection::open(&self.path).context("failed to open sqlite")?;
        connection.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS kv (
              key TEXT PRIMARY KEY,
              value TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS blobs (
              kind TEXT NOT NULL,
              id TEXT NOT NULL,
              value TEXT NOT NULL,
              PRIMARY KEY(kind, id)
            );
        "#,
        )?;
        Ok(())
    }

    pub fn load(&self) -> Result<PersistedState> {
        let connection = Connection::open(&self.path).context("failed to open sqlite")?;
        let mut state = PersistedState::default();

        let config_raw = connection
            .query_row("SELECT value FROM kv WHERE key = 'config'", [], |row| {
                row.get::<_, String>(0)
            })
            .ok();
        if let Some(config_raw) = config_raw {
            state.config = serde_json::from_str(&config_raw).context("failed to parse config")?;
        }

        let manifests_raw = connection
            .query_row("SELECT value FROM kv WHERE key = 'manifests'", [], |row| {
                row.get::<_, String>(0)
            })
            .ok();
        if let Some(manifests_raw) = manifests_raw {
            state.manifests =
                serde_json::from_str(&manifests_raw).context("failed to parse manifests")?;
        }

        state.projects = self.read_blobs(&connection, "project")?;
        state.workspaces = self.read_blobs(&connection, "workspace")?;
        state.services = self.read_blobs(&connection, "service")?;
        state.instances = self.read_blobs(&connection, "instance")?;
        state.routes = self.read_blobs(&connection, "route")?;

        Ok(state)
    }

    pub fn save(&self, state: &PersistedState) -> Result<()> {
        let connection = Connection::open(&self.path).context("failed to open sqlite")?;
        let tx = connection.unchecked_transaction()?;
        tx.execute(
            "INSERT OR REPLACE INTO kv(key, value) VALUES ('config', ?1)",
            params![serde_json::to_string(&state.config)?],
        )?;
        tx.execute(
            "INSERT OR REPLACE INTO kv(key, value) VALUES ('manifests', ?1)",
            params![serde_json::to_string(&state.manifests)?],
        )?;

        for kind in ["project", "workspace", "service", "instance", "route"] {
            tx.execute("DELETE FROM blobs WHERE kind = ?1", params![kind])?;
        }
        self.write_blobs(&tx, "project", &state.projects)?;
        self.write_blobs(&tx, "workspace", &state.workspaces)?;
        self.write_blobs(&tx, "service", &state.services)?;
        self.write_blobs(&tx, "instance", &state.instances)?;
        self.write_blobs(&tx, "route", &state.routes)?;
        tx.commit()?;
        Ok(())
    }

    fn read_blobs<T>(&self, connection: &Connection, kind: &str) -> Result<Vec<T>>
    where
        T: serde::de::DeserializeOwned,
    {
        let mut statement =
            connection.prepare("SELECT value FROM blobs WHERE kind = ?1 ORDER BY id")?;
        let rows = statement.query_map(params![kind], |row| row.get::<_, String>(0))?;
        rows.map(|row| {
            let raw = row?;
            let parsed = serde_json::from_str::<T>(&raw)?;
            Ok(parsed)
        })
        .collect()
    }

    fn write_blobs<T>(&self, tx: &rusqlite::Transaction<'_>, kind: &str, values: &[T]) -> Result<()>
    where
        T: serde::Serialize,
    {
        for value in values {
            let raw = serde_json::to_string(value)?;
            let parsed: serde_json::Value = serde_json::from_str(&raw)?;
            let id = parsed
                .get("id")
                .and_then(serde_json::Value::as_str)
                .unwrap_or(kind)
                .to_string();
            tx.execute(
                "INSERT OR REPLACE INTO blobs(kind, id, value) VALUES (?1, ?2, ?3)",
                params![kind, id, raw],
            )?;
        }
        Ok(())
    }
}
