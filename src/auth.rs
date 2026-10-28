//! Resolve authentication at execution time; inherited credentials are never copied into saved requests.
use crate::model::{Auth, Project, Request};
use anyhow::{Result, bail};
use std::collections::HashSet;
use uuid::Uuid;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AuthOrigin {
    Request,
    Folder(Uuid),
    Workspace,
}
pub struct EffectiveAuth<'a> {
    pub auth: &'a Auth,
    pub origin: AuthOrigin,
}
impl Auth {
    pub fn label(&self) -> &'static str {
        match self {
            Self::None => "无认证",
            Self::Bearer(_) => "Bearer Token",
            Self::Basic { .. } => "Basic Auth",
        }
    }
}
impl Request {
    pub fn inherits_auth(&self) -> bool {
        self.auth_inherit.unwrap_or(matches!(self.auth, Auth::None))
    }
}
impl Project {
    pub fn folder_auth(&self, folder: Option<Uuid>) -> Result<EffectiveAuth<'_>> {
        let mut cursor = folder;
        let mut visited = HashSet::new();
        while let Some(id) = cursor {
            if !visited.insert(id) || visited.len() > 32 {
                bail!("认证继承链存在循环或超过 32 层");
            }
            let folder = self.folder(id)?;
            if let Some(auth) = &folder.auth {
                return Ok(EffectiveAuth {
                    auth,
                    origin: AuthOrigin::Folder(id),
                });
            }
            cursor = folder.parent_id;
        }
        Ok(EffectiveAuth {
            auth: &self.auth,
            origin: AuthOrigin::Workspace,
        })
    }
    pub fn effective_auth<'a>(&'a self, request: &'a Request) -> Result<EffectiveAuth<'a>> {
        if request.inherits_auth() {
            self.folder_auth(request.folder_id)
        } else {
            Ok(EffectiveAuth {
                auth: &request.auth,
                origin: AuthOrigin::Request,
            })
        }
    }
    pub fn auth_origin_label(&self, origin: AuthOrigin) -> String {
        match origin {
            AuthOrigin::Request => "当前请求".into(),
            AuthOrigin::Folder(id) => format!("文件夹 · {}", self.folder_path(id)),
            AuthOrigin::Workspace => format!("工作区 · {}", self.name),
        }
    }
    /// A temporary execution snapshot. Existing runs keep their credentials when settings change.
    pub fn prepare_request(&self, request: &Request) -> Result<Request> {
        let mut result = request.clone();
        result.auth = self.effective_auth(request)?.auth.clone();
        result.auth_inherit = Some(false);
        Ok(result)
    }
}
