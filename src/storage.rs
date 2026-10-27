use crate::{model::Project, transport::RunRecord};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

#[derive(Clone)]
pub struct ProjectStore {
    pub directory: PathBuf,
    revision: Arc<Mutex<u64>>,
}
#[derive(Clone, Debug)]
pub struct ContractInfo {
    pub path: PathBuf,
    pub label: String,
    pub version: String,
}
impl ProjectStore {
    pub fn new(directory: impl Into<PathBuf>) -> Self {
        Self {
            directory: directory.into(),
            revision: Arc::new(Mutex::new(0)),
        }
    }
    pub fn default_directory() -> PathBuf {
        dirs::data_local_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("Zeph/projects/default")
    }
    pub fn load(&self) -> Result<Project> {
        let path = self.directory.join("zeph.project.json");
        if !path.exists() {
            let p = Project::examples();
            self.save(&p)?;
            return Ok(p);
        }
        let mut project: Project = serde_json::from_slice(&fs::read(&path)?)
            .with_context(|| format!("项目无法读取，原文件未修改: {}", path.display()))?;
        project.normalize()?;
        if self.migrate_openapi_groups(&mut project)? {
            self.save(&project)?;
        }
        Ok(project)
    }
    /// Upgrade legacy imported requests using the saved source manifest, never a live network lookup.
    fn migrate_openapi_groups(&self, project: &mut Project) -> Result<bool> {
        let directory = self.directory.join("contracts");
        if !directory.is_dir()
            || !project
                .requests
                .iter()
                .any(|r| r.openapi.is_none() && r.source.is_some())
        {
            return Ok(false);
        }
        let mut changed = false;
        let mut entries = fs::read_dir(directory)?
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .collect::<Vec<_>>();
        entries.sort();
        let mut source_counts = std::collections::HashMap::new();
        for directory in &entries {
            if let Ok(bytes) = fs::read(directory.join("sources.json"))
                && let Ok(manifest) = serde_json::from_slice::<serde_json::Value>(&bytes)
                && let Some(source) = manifest.get("source").and_then(|v| v.as_str())
            {
                *source_counts.entry(source.to_owned()).or_insert(0usize) += 1;
            }
        }
        for directory in entries {
            let Some(id) = directory
                .file_name()
                .and_then(|s| s.to_str())
                .and_then(|s| uuid::Uuid::parse_str(s).ok())
            else {
                continue;
            };
            let read = || -> Result<(serde_json::Value, serde_json::Value)> {
                Ok((
                    serde_json::from_slice(&fs::read(directory.join("original.json"))?)?,
                    serde_json::from_slice(&fs::read(directory.join("sources.json"))?)?,
                ))
            };
            let Ok((document, manifest)) = read() else {
                continue;
            };
            let Some(source) = manifest.get("source").and_then(|v| v.as_str()) else {
                continue;
            };
            // A legacy source string cannot distinguish multiple snapshots of one file.
            if source_counts.get(source) != Some(&1) {
                continue;
            }
            let title = document
                .pointer("/info/title")
                .and_then(|v| v.as_str())
                .unwrap_or("OpenAPI 文档");
            for index in 0..project.requests.len() {
                let request = &project.requests[index];
                if request.openapi.is_some() {
                    continue;
                }
                let Some(suffix) = request
                    .source
                    .as_deref()
                    .and_then(|s| s.strip_prefix(&format!("{source}#")))
                else {
                    continue;
                };
                let Some((method, path)) = suffix.split_once(':') else {
                    continue;
                };
                if ![
                    "get", "post", "put", "patch", "delete", "head", "options", "trace",
                ]
                .contains(&method)
                    || !path.starts_with('/')
                {
                    continue;
                }
                let node = document
                    .get("paths")
                    .and_then(|v| v.get(path))
                    .and_then(|v| v.get(method));
                let tags = node
                    .and_then(|v| v.get("tags"))
                    .and_then(|v| v.as_array())
                    .into_iter()
                    .flatten()
                    .filter_map(|v| v.as_str())
                    .filter(|s| !s.trim().is_empty())
                    .map(str::to_owned)
                    .collect::<Vec<_>>();
                let binding = crate::model::OpenApiOperation {
                    contract_id: id,
                    path: path.into(),
                    method: method.into(),
                    operation_id: node
                        .and_then(|v| v.get("operationId"))
                        .and_then(|v| v.as_str())
                        .map(str::to_owned),
                    tags,
                };
                let folder =
                    project.openapi_folder(id, title, binding.tags.first().map(String::as_str));
                let request = &mut project.requests[index];
                if request.folder_id.is_none() {
                    request.folder_id = Some(folder);
                }
                request.openapi = Some(binding);
                changed = true;
            }
        }
        Ok(changed)
    }
    pub fn create(&self, name: &str) -> Result<Project> {
        if self.directory.join("zeph.project.json").exists() {
            anyhow::bail!("存储位置已有项目，不能覆盖");
        }
        let project = Project::empty(name);
        self.save(&project)?;
        Ok(project)
    }
    pub fn open_existing(directory: impl Into<PathBuf>) -> Result<(Self, Project)> {
        let directory = fs::canonicalize(directory.into())?;
        if !directory.join("zeph.project.json").is_file() {
            anyhow::bail!("工作区数据文件缺失（zeph.project.json）");
        }
        let store = Self::new(directory);
        let project = store.load()?;
        Ok((store, project))
    }
    pub fn save(&self, project: &Project) -> Result<()> {
        fs::create_dir_all(&self.directory)?;
        atomic_write(
            &self.directory.join("zeph.project.json"),
            &serde_json::to_vec_pretty(project)?,
        )
    }
    pub fn save_versioned(&self, project: &Project, revision: u64) -> Result<()> {
        let mut last = self
            .revision
            .lock()
            .map_err(|_| anyhow::anyhow!("项目保存锁不可用"))?;
        if revision < *last {
            return Ok(());
        }
        self.save(project)?;
        *last = revision;
        Ok(())
    }
    pub fn record(&self, record: &RunRecord) -> Result<()> {
        let directory = self.directory.join(".zeph/history");
        fs::create_dir_all(&directory)?;
        atomic_write(
            &directory.join(format!("{}.json", record.id)),
            &serde_json::to_vec_pretty(record)?,
        )
    }
    pub fn history(&self) -> Result<Vec<RunRecord>> {
        let directory = self.directory.join(".zeph/history");
        if !directory.exists() {
            return Ok(vec![]);
        }
        let mut records = vec![];
        for entry in fs::read_dir(directory)? {
            let path = entry?.path();
            if path.extension().is_some_and(|e| e == "json")
                && let Ok(record) = serde_json::from_slice::<RunRecord>(&fs::read(path)?)
            {
                records.push(record);
            }
        }
        records.sort_by_key(|r| std::cmp::Reverse(r.started_at));
        records.truncate(100);
        Ok(records)
    }
    pub fn contracts(&self) -> Result<Vec<ContractInfo>> {
        let directory = self.directory.join("contracts");
        if !directory.exists() {
            return Ok(vec![]);
        }
        let mut contracts = vec![];
        for entry in fs::read_dir(directory)? {
            let path = entry?.path().join("original.json");
            if !path.is_file() {
                continue;
            }
            let document: serde_json::Value = serde_json::from_slice(&fs::read(&path)?)?;
            contracts.push(ContractInfo {
                path,
                label: document
                    .pointer("/info/title")
                    .and_then(|v| v.as_str())
                    .unwrap_or("OpenAPI 文档")
                    .into(),
                version: document
                    .get("openapi")
                    .or_else(|| document.get("swagger"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("未知版本")
                    .into(),
            });
        }
        contracts.sort_by(|a, b| a.path.cmp(&b.path));
        Ok(contracts)
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct WorkspaceSession {
    pub directories: Vec<PathBuf>,
    pub active_directory: Option<PathBuf>,
}
impl WorkspaceSession {
    pub fn default_path() -> PathBuf {
        ProjectStore::default_directory()
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("session.json")
    }
    pub fn load(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let mut session: Self =
            serde_json::from_slice(&fs::read(path)?).context("工作区会话无法读取，原文件未修改")?;
        let mut seen = std::collections::HashSet::new();
        session.directories.retain(|p| seen.insert(p.clone()));
        Ok(session)
    }
    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        atomic_write(path, &serde_json::to_vec_pretty(self)?)
    }
}

/// Application-owned catalog of logical workspaces. Storage locations are internal
/// persistence details; identity and open tabs are always UUIDs.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WorkspaceEntry {
    pub id: uuid::Uuid,
    pub name: String,
    storage_directory: PathBuf,
}
impl WorkspaceEntry {
    pub fn store(&self) -> ProjectStore {
        ProjectStore::new(self.storage_directory.clone())
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct WorkspaceCatalog {
    pub workspaces: Vec<WorkspaceEntry>,
    pub opened: Vec<uuid::Uuid>,
    pub active: Option<uuid::Uuid>,
}
impl WorkspaceCatalog {
    pub fn default_path() -> PathBuf {
        WorkspaceSession::default_path().with_file_name("workspaces.json")
    }
    pub fn load(path: &Path) -> Result<Self> {
        let mut catalog: Self =
            serde_json::from_slice(&fs::read(path)?).context("工作区列表无法读取，原文件未修改")?;
        catalog.normalize()?;
        Ok(catalog)
    }
    /// Bootstrap once, preserving closed legacy workspaces in the logical list.
    /// Existing catalogs are authoritative, including an intentionally empty session.
    pub fn initialize(
        path: &Path,
        legacy: &ProjectStore,
        session: Option<WorkspaceSession>,
    ) -> Result<Self> {
        if path.exists() {
            return Self::load(path);
        }
        let had_session = session.is_some();
        let mut catalog = session
            .map(Self::migrate_session)
            .transpose()?
            .unwrap_or_default();
        if legacy.directory.join("zeph.project.json").is_file() {
            let project = legacy.load()?;
            let id = catalog.adopt(legacy, &project.name);
            if !had_session {
                catalog.open(id)?;
            }
        } else if !had_session {
            let (entry, _) = catalog.create(path, "商城服务")?;
            entry.store().save(&Project::examples())?;
        }
        catalog.save(path)?;
        Ok(catalog)
    }
    pub fn normalize(&mut self) -> Result<()> {
        let mut ids = std::collections::HashSet::new();
        for entry in &self.workspaces {
            if !ids.insert(entry.id) {
                anyhow::bail!("工作区列表包含重复 ID，原文件未修改");
            }
        }
        let mut opened = std::collections::HashSet::new();
        self.opened
            .retain(|id| ids.contains(id) && opened.insert(*id));
        if self.active.is_none_or(|id| !self.opened.contains(&id)) {
            self.active = self.opened.last().copied();
        }
        Ok(())
    }
    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        atomic_write(path, &serde_json::to_vec_pretty(self)?)
    }
    pub fn entry(&self, id: uuid::Uuid) -> Result<&WorkspaceEntry> {
        self.workspaces
            .iter()
            .find(|entry| entry.id == id)
            .context("工作区不存在")
    }
    pub fn open(&mut self, id: uuid::Uuid) -> Result<()> {
        self.entry(id)?;
        if !self.opened.contains(&id) {
            self.opened.push(id);
        }
        self.active = Some(id);
        Ok(())
    }
    pub fn close(&mut self, id: uuid::Uuid) {
        let index = self.opened.iter().position(|value| *value == id);
        self.opened.retain(|value| *value != id);
        if self.active == Some(id) {
            self.active = index.and_then(|index| {
                self.opened
                    .get(index.min(self.opened.len().saturating_sub(1)))
                    .copied()
            });
        }
    }
    /// Allocate private storage without requiring a user-selected folder.
    pub fn create(&mut self, catalog_path: &Path, name: &str) -> Result<(WorkspaceEntry, Project)> {
        let name = name.trim();
        if name.is_empty() {
            anyhow::bail!("请输入工作区名称");
        }
        let id = uuid::Uuid::new_v4();
        let entry = WorkspaceEntry {
            id,
            name: name.into(),
            storage_directory: catalog_path
                .parent()
                .context("工作区存储位置不可用")?
                .join("workspaces")
                .join(id.to_string()),
        };
        let project = entry.store().create(name)?;
        self.workspaces.push(entry.clone());
        self.open(id)?;
        Ok((entry, project))
    }
    /// Register a legacy project once; its files stay in place during migration.
    pub fn adopt(&mut self, store: &ProjectStore, name: &str) -> uuid::Uuid {
        let directory =
            fs::canonicalize(&store.directory).unwrap_or_else(|_| store.directory.clone());
        if let Some(entry) = self
            .workspaces
            .iter()
            .find(|e| e.storage_directory == directory)
        {
            return entry.id;
        }
        let id = uuid::Uuid::new_v4();
        self.workspaces.push(WorkspaceEntry {
            id,
            name: name.into(),
            storage_directory: directory,
        });
        id
    }
    pub fn migrate_session(session: WorkspaceSession) -> Result<Self> {
        let mut catalog = Self::default();
        let active = session
            .active_directory
            .as_ref()
            .map(|path| fs::canonicalize(path).unwrap_or_else(|_| path.clone()));
        for directory in session.directories {
            let store = ProjectStore::new(directory);
            let name = if store.directory.join("zeph.project.json").is_file() {
                store.load()?.name
            } else {
                "未能读取的工作区".into()
            };
            let id = catalog.adopt(&store, &name);
            catalog.open(id)?;
        }
        // Opening later entries must not replace the old active identity.
        catalog.active = active
            .and_then(|path| {
                catalog
                    .workspaces
                    .iter()
                    .find(|e| e.storage_directory == path)
                    .map(|e| e.id)
            })
            .or_else(|| catalog.opened.last().copied());
        Ok(catalog)
    }
}

fn atomic_write(path: &Path, bytes: &[u8]) -> Result<()> {
    let temp = path.with_extension(format!("{}.tmp", uuid::Uuid::new_v4()));
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let result = (|| -> Result<()> {
        let mut file = options.open(&temp)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        fs::rename(&temp, path)?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(temp);
    }
    result
}

/// Retain the root document and every local reference read during import.
/// The manifest maps original paths to unmodified documents, without fetching remote refs.
pub fn save_contract(directory: &Path, import: &crate::openapi::ImportedSpec) -> Result<()> {
    let directory = directory.join(import.contract_id.to_string());
    fs::create_dir_all(&directory)?;
    atomic_write(
        &directory.join("original.json"),
        &serde_json::to_vec_pretty(&import.document)?,
    )?;
    let files = import
        .documents
        .iter()
        .map(|(path, document)| serde_json::json!({"path":path,"document":document}))
        .collect::<Vec<_>>();
    let manifest = serde_json::json!({"contractId":import.contract_id,"source":import.source,"version":import.version,"files":files,"diagnostics":import.diagnostics});
    atomic_write(
        &directory.join("sources.json"),
        &serde_json::to_vec_pretty(&manifest)?,
    )
}
