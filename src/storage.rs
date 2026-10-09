use crate::{model::Project, transport::RunRecord};
use anyhow::{Context, Result};
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
        Ok(project)
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
    let directory = directory.join(uuid::Uuid::new_v4().to_string());
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
    let manifest = serde_json::json!({"source":import.source,"version":import.version,"files":files,"diagnostics":import.diagnostics});
    atomic_write(
        &directory.join("sources.json"),
        &serde_json::to_vec_pretty(&manifest)?,
    )
}
