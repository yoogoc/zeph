//! Application preferences are independent from logical workspaces.
use crate::{model::Request, updater};
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use std::{
    io::Write,
    path::{Path, PathBuf},
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Appearance {
    #[default]
    System,
    Light,
    Dark,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct RequestDefaults {
    pub timeout_secs: u64,
    pub verify_tls: bool,
    pub proxy: String,
}
impl Default for RequestDefaults {
    fn default() -> Self {
        Self {
            timeout_secs: 30,
            verify_tls: true,
            proxy: String::new(),
        }
    }
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Preferences {
    pub appearance: Appearance,
    pub requests: RequestDefaults,
    pub updates: updater::Preferences,
    pub github_token_configured: bool,
}
impl Preferences {
    pub fn validate(&self) -> Result<()> {
        ensure!(
            (1..=3600).contains(&self.requests.timeout_secs),
            "请求超时必须为 1–3600 秒"
        );
        if !self.requests.proxy.trim().is_empty() {
            let url =
                url::Url::parse(self.requests.proxy.trim()).context("请输入有效的代理地址")?;
            ensure!(
                matches!(url.scheme(), "http" | "https") && url.host_str().is_some(),
                "代理需要使用 http:// 或 https:// 地址"
            );
            ensure!(
                url.username().is_empty() && url.password().is_none(),
                "应用代理配置不保存认证信息，请在请求中单独配置"
            );
        }
        Ok(())
    }
    pub fn apply_request_defaults(&self, request: &mut Request) {
        request.timeout_secs = self.requests.timeout_secs;
        request.tls_verify = self.requests.verify_tls;
        request.proxy = self.requests.proxy.clone();
    }
    pub fn load(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let p: Self = serde_json::from_slice(&std::fs::read(path)?)
            .context("应用配置无法读取，原文件未修改")?;
        p.validate()?;
        Ok(p)
    }
    pub fn save(&self, path: &Path) -> Result<()> {
        self.validate()?;
        let parent = path.parent().context("应用配置目录不存在")?;
        std::fs::create_dir_all(parent)?;
        let mut temp = tempfile::NamedTempFile::new_in(parent)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            temp.as_file()
                .set_permissions(std::fs::Permissions::from_mode(0o600))?;
        }
        temp.write_all(&serde_json::to_vec_pretty(self)?)?;
        temp.as_file().sync_all()?;
        temp.persist(path).map_err(|e| e.error)?;
        Ok(())
    }
    pub fn directory() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("Zeph")
    }
}

#[cfg(feature = "credentials")]
fn entry() -> Result<keyring::Entry> {
    keyring::Entry::new("dev.zeph.updates", "github")
        .map_err(|_| anyhow::anyhow!("系统凭据存储不可用"))
}
#[cfg(feature = "credentials")]
pub fn update_token(token: Option<&str>) -> Result<()> {
    let e = entry()?;
    match token {
        Some(token) => {
            ensure!(!token.trim().is_empty(), "请输入 GitHub Token");
            e.set_password(token.trim())
                .map_err(|_| anyhow::anyhow!("GitHub Token 无法保存到系统凭据存储"))
        }
        None => match e.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(_) => anyhow::bail!("无法移除 GitHub Token"),
        },
    }
}
#[cfg(feature = "credentials")]
pub fn read_token() -> Result<zeroize::Zeroizing<String>> {
    entry()?
        .get_password()
        .map(zeroize::Zeroizing::new)
        .map_err(|_| anyhow::anyhow!("GitHub Token 不可用，请重新保存"))
}
#[cfg(not(feature = "credentials"))]
pub fn update_token(_: Option<&str>) -> Result<()> {
    anyhow::bail!("此构建未启用系统凭据存储")
}
#[cfg(not(feature = "credentials"))]
pub fn read_token() -> Result<zeroize::Zeroizing<String>> {
    anyhow::bail!("此构建未启用系统凭据存储")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn partial_settings_migrate_and_are_separate_from_request_drafts() {
        let p: Preferences = serde_json::from_str(r#"{"appearance":"dark"}"#).unwrap();
        assert!(p.updates.auto_check);
        assert!(!p.updates.auto_download);
        assert!(p.requests.verify_tls);
        let mut r = Request::new(crate::model::Protocol::Http, "Test");
        r.timeout_secs = 77;
        let mut p = p;
        p.requests.timeout_secs = 15;
        assert_eq!(r.timeout_secs, 77);
        p.apply_request_defaults(&mut r);
        assert_eq!(r.timeout_secs, 15);
    }
    #[test]
    fn settings_are_atomic_validated_and_do_not_contain_credentials() {
        let t = tempfile::tempdir().unwrap();
        let path = t.path().join("settings.json");
        let mut p = Preferences::default();
        p.save(&path).unwrap();
        let original = std::fs::read(&path).unwrap();
        p.requests.timeout_secs = 0;
        assert!(p.save(&path).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), original);
        assert!(Preferences::load(&path).unwrap().requests.verify_tls);
        let value: serde_json::Value = serde_json::from_slice(&original).unwrap();
        assert!(value.get("token").is_none());
        p.requests.timeout_secs = 30;
        p.requests.proxy = "https://user:password@proxy.test".into();
        assert!(p.validate().is_err());
        std::fs::write(&path, b"broken").unwrap();
        assert!(Preferences::load(&path).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), b"broken");
    }
}
