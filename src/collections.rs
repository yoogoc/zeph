//! Logical collection folders. IDs and OpenAPI bindings survive display-name changes.
use crate::model::{Folder, OpenApiFolder, Project};
use anyhow::{Result, bail};
use std::collections::HashSet;
use uuid::Uuid;

#[derive(Clone, Debug)]
pub enum CollectionRow {
    Folder {
        id: Uuid,
        depth: usize,
        count: usize,
    },
    Request {
        index: usize,
        depth: usize,
    },
}
#[derive(Clone)]
pub struct DeletedFolder {
    folder: Folder,
    index: usize,
    requests: Vec<Uuid>,
    children: Vec<Uuid>,
}
impl Project {
    pub fn merge_collection(
        &mut self,
        folders: Vec<Folder>,
        requests: Vec<crate::model::Request>,
    ) -> Result<()> {
        let mut next = self.clone();
        for mut folder in folders {
            if next.folders.iter().any(|f| f.id == folder.id) {
                bail!("导入含有重复文件夹 ID");
            }
            if let Some(parent) = folder.parent_id {
                next.folder(parent)?;
            }
            let original = folder.name.clone();
            let mut suffix = 2;
            while next
                .check_folder_name(&folder.name, folder.parent_id, None)
                .is_err()
            {
                folder.name = format!("{original} ({suffix})");
                suffix += 1;
            }
            next.folders.push(folder);
        }
        next.requests.extend(requests);
        next.normalize()?;
        *self = next;
        Ok(())
    }
    pub fn folder(&self, id: Uuid) -> Result<&Folder> {
        self.folders
            .iter()
            .find(|f| f.id == id)
            .ok_or_else(|| anyhow::anyhow!("文件夹不存在"))
    }
    pub fn folder_path(&self, id: Uuid) -> String {
        let mut names = Vec::new();
        let mut cursor = Some(id);
        let mut seen = HashSet::new();
        while let Some(id) = cursor {
            if !seen.insert(id) {
                break;
            }
            let Ok(folder) = self.folder(id) else {
                break;
            };
            names.push(folder.name.as_str());
            cursor = folder.parent_id;
        }
        names.reverse();
        names.join(" / ")
    }
    pub fn folder_contains(&self, ancestor: Uuid, candidate: Uuid) -> bool {
        let mut cursor = Some(candidate);
        let mut seen = HashSet::new();
        while let Some(id) = cursor {
            if id == ancestor {
                return true;
            }
            if !seen.insert(id) {
                return false;
            }
            cursor = self.folder(id).ok().and_then(|f| f.parent_id);
        }
        false
    }
    pub fn validate_folders(&self) -> Result<()> {
        let mut ids = HashSet::new();
        let mut names = HashSet::new();
        for folder in &self.folders {
            if !ids.insert(folder.id) {
                bail!("项目含有重复文件夹 ID，原文件未修改");
            }
            if folder.name.trim().is_empty() {
                bail!("文件夹名称不能为空");
            }
            if !names.insert((folder.parent_id, folder.name.as_str())) {
                bail!("同一层级已有同名文件夹");
            }
        }
        for folder in &self.folders {
            let mut cursor = Some(folder.id);
            let mut visited = HashSet::new();
            while let Some(id) = cursor {
                if !visited.insert(id) {
                    bail!("文件夹不能包含自身或形成循环");
                }
                if visited.len() > 32 {
                    bail!("文件夹嵌套不能超过 32 层");
                }
                cursor = self.folder(id)?.parent_id;
            }
        }
        for request in &self.requests {
            if let Some(id) = request.folder_id {
                self.folder(id)?;
            }
        }
        Ok(())
    }
    fn check_folder_name(
        &self,
        name: &str,
        parent: Option<Uuid>,
        except: Option<Uuid>,
    ) -> Result<()> {
        if name.trim().is_empty() {
            bail!("文件夹名称不能为空");
        }
        if let Some(id) = parent {
            self.folder(id)?;
        }
        if self
            .folders
            .iter()
            .any(|f| f.parent_id == parent && Some(f.id) != except && f.name == name.trim())
        {
            bail!("同一层级已有同名文件夹");
        }
        Ok(())
    }
    pub fn add_folder(&mut self, name: &str, parent: Option<Uuid>) -> Result<Uuid> {
        self.check_folder_name(name, parent, None)?;
        let folder = Folder {
            id: Uuid::new_v4(),
            name: name.trim().into(),
            parent_id: parent,
            expanded: true,
            auth: None,
            openapi: None,
        };
        let id = folder.id;
        self.folders.push(folder);
        if let Err(e) = self.validate_folders() {
            self.folders.pop();
            return Err(e);
        }
        if let Some(parent) = parent {
            self.folders
                .iter_mut()
                .find(|f| f.id == parent)
                .unwrap()
                .expanded = true;
        }
        Ok(id)
    }
    pub fn rename_folder(&mut self, id: Uuid, name: &str) -> Result<()> {
        let parent = self.folder(id)?.parent_id;
        self.check_folder_name(name, parent, Some(id))?;
        self.folders.iter_mut().find(|f| f.id == id).unwrap().name = name.trim().into();
        Ok(())
    }
    pub fn move_folder(&mut self, id: Uuid, parent: Option<Uuid>) -> Result<()> {
        let folder = self.folder(id)?.clone();
        if parent.is_some_and(|p| self.folder_contains(id, p)) {
            bail!("不能将文件夹移入自身或子文件夹");
        }
        self.check_folder_name(&folder.name, parent, Some(id))?;
        self.folders
            .iter_mut()
            .find(|f| f.id == id)
            .unwrap()
            .parent_id = parent;
        if let Err(e) = self.validate_folders() {
            self.folders
                .iter_mut()
                .find(|f| f.id == id)
                .unwrap()
                .parent_id = folder.parent_id;
            return Err(e);
        }
        self.expand_folder_ancestors(parent);
        Ok(())
    }
    pub fn expand_folder_ancestors(&mut self, folder: Option<Uuid>) {
        let mut cursor = folder;
        let mut seen = HashSet::new();
        while let Some(id) = cursor {
            if !seen.insert(id) {
                break;
            }
            let Some(folder) = self.folders.iter_mut().find(|f| f.id == id) else {
                break;
            };
            folder.expanded = true;
            cursor = folder.parent_id;
        }
    }
    pub fn move_request_to_folder(&mut self, id: Uuid, folder: Option<Uuid>) -> Result<()> {
        if let Some(id) = folder {
            self.folder(id)?;
        }
        let request = self
            .requests
            .iter_mut()
            .find(|r| r.id == id)
            .ok_or_else(|| anyhow::anyhow!("请求不存在"))?;
        request.folder_id = folder;
        self.expand_folder_ancestors(folder);
        Ok(())
    }
    /// Remove only the grouping, promoting direct children and keeping requests and jobs intact.
    pub fn remove_folder(&mut self, id: Uuid) -> Result<DeletedFolder> {
        let folder = self.folder(id)?.clone();
        // Promotion must not introduce sibling-name collisions.
        for child in self.folders.iter().filter(|f| f.parent_id == Some(id)) {
            if self.folders.iter().any(|f| {
                f.id != id
                    && f.id != child.id
                    && f.parent_id == folder.parent_id
                    && f.name == child.name
            }) {
                bail!("上一级已有同名文件夹，请先重命名或移动子文件夹");
            }
        }
        let index = self.folders.iter().position(|f| f.id == id).unwrap();
        self.folders.remove(index);
        let mut removed = DeletedFolder {
            folder,
            index,
            requests: vec![],
            children: vec![],
        };
        for request in &mut self.requests {
            if request.folder_id == Some(id) {
                removed.requests.push(request.id);
                request.folder_id = removed.folder.parent_id;
            }
        }
        for child in &mut self.folders {
            if child.parent_id == Some(id) {
                removed.children.push(child.id);
                child.parent_id = removed.folder.parent_id;
            }
        }
        Ok(removed)
    }
    pub fn restore_folder(&mut self, removed: DeletedFolder) -> Result<Uuid> {
        let mut next = self.clone();
        let id = next.restore_folder_inner(removed)?;
        next.validate_folders()?;
        *self = next;
        Ok(id)
    }
    fn restore_folder_inner(&mut self, removed: DeletedFolder) -> Result<Uuid> {
        let mut folder = removed.folder;
        if self.folders.iter().any(|f| f.id == folder.id) {
            bail!("文件夹已恢复");
        }
        if folder.parent_id.is_some_and(|id| self.folder(id).is_err()) {
            folder.parent_id = None;
        }
        while self
            .check_folder_name(&folder.name, folder.parent_id, None)
            .is_err()
        {
            folder.name.push_str(" 恢复");
        }
        let id = folder.id;
        let parent = folder.parent_id;
        self.folders
            .insert(removed.index.min(self.folders.len()), folder);
        for request in &mut self.requests {
            if removed.requests.contains(&request.id) && request.folder_id == parent {
                request.folder_id = Some(id);
            }
        }
        let restore_children = self
            .folders
            .iter()
            .filter(|child| {
                removed.children.contains(&child.id)
                    && child.parent_id == parent
                    && !self.folder_contains(child.id, id)
            })
            .map(|f| f.id)
            .collect::<HashSet<_>>();
        for child in &mut self.folders {
            if restore_children.contains(&child.id) {
                child.parent_id = Some(id);
            }
        }
        self.expand_folder_ancestors(Some(id));
        Ok(id)
    }
    /// A document root and one flat child per OpenAPI tag; tag strings are never split on '/'.
    pub fn openapi_directory(
        &mut self,
        contract_id: Uuid,
        title: &str,
        path: &[String],
    ) -> Result<Uuid> {
        let mut parent = self.openapi_folder(contract_id, title, None);
        for depth in 1..=path.len() {
            let prefix = &path[..depth];
            if let Some(existing) = self.folders.iter().find(|f| {
                f.openapi.as_ref().is_some_and(|m| {
                    m.contract_id == contract_id && m.directory.as_deref() == Some(prefix)
                })
            }) {
                parent = existing.id;
                continue;
            }
            let base = &path[depth - 1];
            let mut name = base.clone();
            let mut suffix = 2;
            while self.check_folder_name(&name, Some(parent), None).is_err() {
                name = format!("{base} ({suffix})");
                suffix += 1;
            }
            let id = self.add_folder(&name, Some(parent))?;
            self.folders
                .iter_mut()
                .find(|f| f.id == id)
                .unwrap()
                .openapi = Some(OpenApiFolder {
                contract_id,
                tag: None,
                directory: Some(prefix.to_vec()),
            });
            parent = id;
        }
        Ok(parent)
    }
    pub fn openapi_folder(&mut self, contract_id: Uuid, title: &str, tag: Option<&str>) -> Uuid {
        if let Some(folder) = self.folders.iter().find(|f| {
            f.openapi.as_ref().is_some_and(|m| {
                m.contract_id == contract_id && m.tag.as_deref() == tag && m.directory.is_none()
            })
        }) {
            return folder.id;
        }
        let parent_id = tag.map(|_| self.openapi_folder(contract_id, title, None));
        let base = tag.unwrap_or(title).trim();
        let mut name = if base.is_empty() {
            "OpenAPI 文档".into()
        } else {
            base.to_owned()
        };
        let original = name.clone();
        let mut suffix = 2;
        while self.check_folder_name(&name, parent_id, None).is_err() {
            name = format!("{original} ({suffix})");
            suffix += 1;
        }
        let id = self
            .add_folder(&name, parent_id)
            .expect("valid OpenAPI folder at depth <= 2");
        self.folders
            .iter_mut()
            .find(|f| f.id == id)
            .unwrap()
            .openapi = Some(OpenApiFolder {
            contract_id,
            tag: tag.map(str::to_owned),
            directory: None,
        });
        id
    }
    pub fn collection_rows(&self, query: &str) -> Vec<CollectionRow> {
        fn visit(
            project: &Project,
            parent: Option<Uuid>,
            depth: usize,
            query: &str,
            inherited: bool,
        ) -> Vec<CollectionRow> {
            if depth > 32 {
                return vec![];
            }
            let mut rows = Vec::new();
            for folder in project.folders.iter().filter(|f| f.parent_id == parent) {
                let matches =
                    inherited || (!query.is_empty() && folder.name.to_lowercase().contains(query));
                let children = visit(project, Some(folder.id), depth + 1, query, matches);
                if query.is_empty() || matches || !children.is_empty() {
                    let count = project
                        .requests
                        .iter()
                        .filter(|r| {
                            r.folder_id
                                .is_some_and(|id| project.folder_contains(folder.id, id))
                        })
                        .count();
                    rows.push(CollectionRow::Folder {
                        id: folder.id,
                        depth,
                        count,
                    });
                    if folder.expanded || !query.is_empty() {
                        rows.extend(children);
                    }
                }
            }
            for (index, request) in project
                .requests
                .iter()
                .enumerate()
                .filter(|(_, r)| r.folder_id == parent)
            {
                let tags = request
                    .openapi
                    .as_ref()
                    .map(|m| m.tags.join(" "))
                    .unwrap_or_default();
                if query.is_empty()
                    || inherited
                    || format!(
                        "{} {} {} {} {tags}",
                        request.name,
                        request.method,
                        request.protocol.label(),
                        request.url
                    )
                    .to_lowercase()
                    .contains(query)
                {
                    rows.push(CollectionRow::Request { index, depth });
                }
            }
            rows
        }
        visit(self, None, 0, &query.to_lowercase(), false)
    }
}
