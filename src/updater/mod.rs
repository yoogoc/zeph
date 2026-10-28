//! Signed GitHub updates. Network work belongs on Tokio; installation runs in
//! a copied helper process only after the GUI has released its exit lock.
mod install;

use anyhow::{Context, Result, bail};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use minisign_verify::{PublicKey, Signature};
use semver::Version;
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, path::PathBuf, time::Duration};
use tempfile::TempDir;
use url::Url;

pub use install::{
    Installation, PreparedInstall, helper_main, prepare_install, read_install_result,
};

pub const REPOSITORY: &str = "yoogoc/zeph";
pub const PUBLIC_KEY: &str = match option_env!("ZEPH_UPDATE_PUBLIC_KEY") {
    Some(key) => {
        if key.is_empty() {
            include_str!("../../assets/packaging/update-public-key")
        } else {
            key
        }
    }
    None => include_str!("../../assets/packaging/update-public-key"),
};
const MAX_METADATA: u64 = 2 * 1024 * 1024;
const MAX_PACKAGE: u64 = 2 * 1024 * 1024 * 1024;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Channel {
    #[default]
    Stable,
    Development,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Preferences {
    pub auto_check: bool,
    pub auto_download: bool,
    pub channel: Channel,
}
impl Default for Preferences {
    fn default() -> Self {
        Self {
            auto_check: true,
            auto_download: false,
            channel: Channel::Stable,
        }
    }
}

pub enum Proxy {
    System,
    Direct,
    Custom(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Format {
    App,
    Nsis,
    Appimage,
    Deb,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Asset {
    pub url: Url,
    /// cargo-packager's base64-encoded Minisign signature text.
    pub signature: String,
    pub size: u64,
    pub format: Format,
}
#[derive(Debug, Deserialize)]
pub struct Manifest {
    pub version: Version,
    pub platforms: BTreeMap<String, Asset>,
}
#[derive(Debug, Clone)]
pub struct Release {
    pub version: Version,
    pub notes: String,
    pub page: Url,
    pub asset: Asset,
    pub api_url: Option<Url>,
}
#[derive(Deserialize)]
struct GithubRelease {
    tag_name: String,
    draft: bool,
    prerelease: bool,
    body: Option<String>,
    html_url: Url,
    assets: Vec<GithubAsset>,
}
#[derive(Deserialize)]
struct GithubAsset {
    name: String,
    browser_download_url: Url,
    url: Url,
}

pub struct Client {
    http: reqwest::Client,
    key: PublicKey,
    token: Option<zeroize::Zeroizing<String>>,
}
impl Client {
    pub fn new(proxy: Proxy, key: &str) -> Result<Self> {
        let key = public_key(key)?;
        let mut builder = reqwest::Client::builder()
            .user_agent(concat!("Zeph/", env!("CARGO_PKG_VERSION")))
            .connect_timeout(Duration::from_secs(20))
            .read_timeout(Duration::from_secs(60))
            .redirect(reqwest::redirect::Policy::custom(|attempt| {
                if attempt.previous().len() >= 8 || attempt.url().scheme() != "https" {
                    attempt.error("Unsafe update redirect")
                } else {
                    attempt.follow()
                }
            }));
        builder = match proxy {
            Proxy::System => builder,
            Proxy::Direct => builder.no_proxy(),
            Proxy::Custom(url) => builder.no_proxy().proxy(reqwest::Proxy::all(url)?),
        };
        Ok(Self {
            http: builder.build()?,
            key,
            token: None,
        })
    }
    pub fn with_token(mut self, token: Option<zeroize::Zeroizing<String>>) -> Self {
        self.token = token.filter(|t| !t.trim().is_empty());
        self
    }
    fn request(&self, url: Url) -> reqwest::RequestBuilder {
        let authenticated = url.scheme() == "https"
            && url.host_str() == Some("api.github.com")
            && url.path().starts_with(&format!("/repos/{REPOSITORY}/"));
        let mut request = self.http.get(url.clone());
        if authenticated {
            if let Some(token) = &self.token {
                request = request.bearer_auth(token.as_str());
            }
            if url.path().contains("/releases/assets/") {
                request = request.header("Accept", "application/octet-stream");
            }
        }
        request
    }
    fn asset_location(&self, asset: &GithubAsset, tag: &str) -> Result<Url> {
        validate_asset_url(&asset.browser_download_url, tag)?;
        if self.token.is_some() {
            validate_api_asset_url(&asset.url)?;
            Ok(asset.url.clone())
        } else {
            Ok(asset.browser_download_url.clone())
        }
    }
    async fn bytes(&self, url: Url, limit: u64) -> Result<Vec<u8>> {
        let mut response = self
            .request(url)
            .timeout(Duration::from_secs(60))
            .send()
            .await?
            .error_for_status()?;
        if response.content_length().is_some_and(|n| n > limit) {
            bail!("Update metadata is too large");
        }
        let mut result = Vec::new();
        while let Some(chunk) = response.chunk().await? {
            if result.len() as u64 + chunk.len() as u64 > limit {
                bail!("Update metadata is too large");
            }
            result.extend_from_slice(&chunk);
        }
        Ok(result)
    }
    pub async fn check(
        &self,
        current: &str,
        channel: Channel,
        installation: &Installation,
    ) -> Result<Option<Release>> {
        let current = Version::parse(current)?;
        let api = match channel {
            Channel::Stable => format!("https://api.github.com/repos/{REPOSITORY}/releases/latest"),
            Channel::Development => {
                format!("https://api.github.com/repos/{REPOSITORY}/releases?per_page=100")
            }
        };
        let response = self
            .request(Url::parse(&api)?)
            .timeout(Duration::from_secs(60))
            .header("Accept", "application/vnd.github+json")
            .send()
            .await?;
        if response.status() == reqwest::StatusCode::NOT_FOUND {
            bail!("此仓库尚未发布可用版本，请稍后重试或前往 Releases 查看");
        }
        let response = response.error_for_status()?;
        // Bound GitHub metadata as well as the signed manifest.
        let mut response = response;
        let mut bytes = Vec::new();
        while let Some(chunk) = response.chunk().await? {
            if bytes.len() as u64 + chunk.len() as u64 > MAX_METADATA {
                bail!("Release metadata is too large");
            }
            bytes.extend_from_slice(&chunk);
        }
        let releases: Vec<GithubRelease> = if channel == Channel::Stable {
            vec![serde_json::from_slice(&bytes)?]
        } else {
            serde_json::from_slice(&bytes)?
        };
        let mut candidates: Vec<_> = releases
            .into_iter()
            .filter_map(|release| {
                let version = Version::parse(release.tag_name.strip_prefix('v')?).ok()?;
                eligible(
                    &current,
                    &version,
                    channel,
                    release.draft,
                    release.prerelease,
                )
                .then_some((version, release))
            })
            .collect();
        candidates.sort_by(|a, b| b.0.cmp(&a.0));
        let Some((version, release)) = candidates.into_iter().next() else {
            return Ok(None);
        };
        let find = |name: &str| {
            release.assets.iter().find(|a| a.name == name)
            .with_context(|| format!("Release {version} has no signed update manifest; download this release manually"))
        };
        let manifest_url = self.asset_location(find("update.json")?, &release.tag_name)?;
        let signature_url = self.asset_location(find("update.json.sig")?, &release.tag_name)?;
        let bytes = self.bytes(manifest_url, MAX_METADATA).await?;
        let signature = self.bytes(signature_url, 16 * 1024).await?;
        verify(&self.key, &bytes, std::str::from_utf8(&signature)?.trim())?;
        let manifest: Manifest = serde_json::from_slice(&bytes)?;
        if manifest.version != version {
            bail!("Signed manifest version does not match the release");
        }
        let platform = installation.platform();
        let asset = manifest
            .platforms
            .get(&platform)
            .with_context(|| format!("No update package is available for {platform}"))?
            .clone();
        validate_asset_url(&asset.url, &release.tag_name)?;
        if asset.size == 0 || asset.size > MAX_PACKAGE || !installation.accepts(&asset.format) {
            bail!("Invalid update package size or format");
        }
        let api_url = if self.token.is_some() {
            let package = release
                .assets
                .iter()
                .find(|a| a.browser_download_url == asset.url)
                .context("更新包未包含在所选 Release 中")?;
            validate_api_asset_url(&package.url)?;
            Some(package.url.clone())
        } else {
            None
        };
        Ok(Some(Release {
            version,
            notes: release.body.unwrap_or_default(),
            page: release.html_url,
            asset,
            api_url,
        }))
    }
    /// TempDir is dropped on cancellation/failure, removing partial packages.
    pub async fn download(
        &self,
        release: Release,
        cache: PathBuf,
        mut progress: impl FnMut(u64, u64) + Send,
    ) -> Result<Downloaded> {
        // Synchronous file handles are confined to the network worker. Their
        // deterministic drop closes the file before TempDir cleanup on Windows.
        use std::io::Write as _;
        tokio::fs::create_dir_all(&cache).await?;
        let directory = tempfile::Builder::new()
            .prefix("download-")
            .tempdir_in(cache)?;
        let path = directory.path().join("package");
        let mut file = std::fs::File::create(&path)?;
        let mut response = self
            .request(
                release
                    .api_url
                    .clone()
                    .unwrap_or_else(|| release.asset.url.clone()),
            )
            .timeout(Duration::from_secs(60 * 60))
            .send()
            .await?
            .error_for_status()?;
        if response
            .content_length()
            .is_some_and(|n| n != release.asset.size)
        {
            bail!("Update package size does not match the signed manifest");
        }
        let signature = signature(&release.asset.signature)?;
        let mut verifier = self
            .key
            .verify_stream(&signature)
            .context("Invalid update signature")?;
        let mut received = 0u64;
        let mut last = std::time::Instant::now();
        while let Some(chunk) = response.chunk().await? {
            received = received
                .checked_add(chunk.len() as u64)
                .context("Update size overflow")?;
            if received > release.asset.size {
                bail!("Update package exceeds its signed size");
            }
            file.write_all(&chunk)?;
            verifier.update(&chunk);
            if last.elapsed() >= Duration::from_millis(100) {
                progress(received, release.asset.size);
                last = std::time::Instant::now();
            }
        }
        if received != release.asset.size {
            bail!("Update download is incomplete");
        }
        verifier
            .finalize()
            .context("Update signature verification failed")?;
        file.sync_all()?;
        drop(file);
        progress(received, release.asset.size);
        Ok(Downloaded {
            release,
            directory: std::sync::Mutex::new(directory),
            path,
        })
    }
}

pub struct Downloaded {
    pub release: Release,
    pub path: PathBuf,
    pub(crate) directory: std::sync::Mutex<TempDir>,
}

fn public_key(encoded: &str) -> Result<PublicKey> {
    if encoded.trim().is_empty() {
        bail!(
            "This build has no update verification key. Install an official Zeph release to enable automatic updates."
        );
    }
    let text = String::from_utf8(
        STANDARD
            .decode(encoded.trim())
            .context("Invalid update public key")?,
    )?;
    PublicKey::decode(&text).context("Invalid update public key")
}
fn signature(encoded: &str) -> Result<Signature> {
    let text = String::from_utf8(
        STANDARD
            .decode(encoded.trim())
            .context("Invalid signature encoding")?,
    )?;
    Signature::decode(&text).context("Invalid update signature")
}
fn verify(key: &PublicKey, bytes: &[u8], encoded: &str) -> Result<()> {
    key.verify(bytes, &signature(encoded)?, false)
        .context("Update signature verification failed")
}
pub(crate) fn verify_file(path: &std::path::Path, encoded: &str) -> Result<()> {
    verify_package(PUBLIC_KEY, path, encoded)
}
pub fn verify_package(key: &str, path: &std::path::Path, encoded: &str) -> Result<()> {
    use std::io::Read as _;
    let key = public_key(key)?;
    let sig = signature(encoded)?;
    let mut verifier = key.verify_stream(&sig)?;
    let mut file = std::fs::File::open(path)?;
    let mut buffer = [0; 65536];
    loop {
        let n = file.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        verifier.update(&buffer[..n]);
    }
    verifier
        .finalize()
        .context("Update package changed after verification")
}
fn eligible(
    current: &Version,
    candidate: &Version,
    channel: Channel,
    draft: bool,
    prerelease: bool,
) -> bool {
    !draft
        && candidate > current
        && (channel == Channel::Development || (!prerelease && candidate.pre.is_empty()))
}
fn validate_api_asset_url(url: &Url) -> Result<()> {
    let prefix = format!("/repos/{REPOSITORY}/releases/assets/");
    if url.scheme() != "https"
        || url.host_str() != Some("api.github.com")
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || !url
            .path()
            .strip_prefix(&prefix)
            .is_some_and(|id| !id.is_empty() && id.bytes().all(|b| b.is_ascii_digit()))
    {
        bail!("更新包 API 地址不属于所选仓库");
    }
    Ok(())
}
fn validate_asset_url(url: &Url, tag: &str) -> Result<()> {
    let prefix = format!("/{REPOSITORY}/releases/download/{tag}/");
    if url.scheme() != "https"
        || url.host_str() != Some("github.com")
        || !url.path().starts_with(&prefix)
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || url.path()[prefix.len()..].contains('/')
        || url.path()[prefix.len()..].is_empty()
    {
        bail!("Update URL is outside the expected GitHub release");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn embedded_key_and_private_repository_credentials_are_scoped() {
        let client = Client::new(Proxy::Direct, PUBLIC_KEY)
            .unwrap()
            .with_token(Some(zeroize::Zeroizing::new("test-secret".into())));
        for (url, authenticated) in [
            (
                "https://api.github.com/repos/yoogoc/zeph/releases/latest",
                true,
            ),
            (
                "https://api.github.com/repos/yoogoc/other/releases/latest",
                false,
            ),
            (
                "https://github.com/yoogoc/zeph/releases/download/v1.0.0/package",
                false,
            ),
            ("https://example.com/package", false),
            (
                "http://api.github.com/repos/yoogoc/zeph/releases/latest",
                false,
            ),
        ] {
            let request = client.request(Url::parse(url).unwrap()).build().unwrap();
            assert_eq!(
                request
                    .headers()
                    .contains_key(reqwest::header::AUTHORIZATION),
                authenticated
            );
        }
    }
    #[test]
    fn private_asset_endpoints_cannot_escape_the_repository() {
        assert!(
            validate_api_asset_url(
                &Url::parse("https://api.github.com/repos/yoogoc/zeph/releases/assets/123")
                    .unwrap()
            )
            .is_ok()
        );
        for value in [
            "https://evil.test/repos/yoogoc/zeph/releases/assets/123",
            "https://api.github.com/repos/other/repo/releases/assets/123",
            "https://api.github.com/repos/yoogoc/zeph/releases/assets/not-an-id",
            "https://api.github.com/repos/yoogoc/zeph/releases/assets/123?token=abc",
        ] {
            assert!(validate_api_asset_url(&Url::parse(value).unwrap()).is_err());
        }
    }
    #[test]
    fn settings_migrate_with_safe_defaults() {
        let prefs: Preferences = serde_json::from_str("{}").unwrap();
        assert!(prefs.auto_check);
        assert!(!prefs.auto_download);
        assert_eq!(prefs.channel, Channel::Stable);
    }
    #[test]
    fn update_order_never_downgrades_or_installs_a_draft() {
        let v = |s| Version::parse(s).unwrap();
        assert!(!eligible(
            &v("0.2.7"),
            &v("0.2.7-dev.20"),
            Channel::Development,
            false,
            true
        ));
        assert!(eligible(
            &v("0.2.7"),
            &v("0.2.8-dev.20"),
            Channel::Development,
            false,
            true
        ));
        assert!(!eligible(
            &v("0.2.7"),
            &v("0.2.8-dev.20"),
            Channel::Stable,
            false,
            true
        ));
        assert!(eligible(
            &v("0.2.8-dev.20"),
            &v("0.2.8"),
            Channel::Stable,
            false,
            false
        ));
        assert!(!eligible(
            &v("0.2.7"),
            &v("0.2.8"),
            Channel::Stable,
            true,
            false
        ));
        assert!(!eligible(
            &v("0.2.7"),
            &v("0.2.7"),
            Channel::Stable,
            false,
            false
        ));
    }
    #[test]
    fn assets_must_belong_to_the_selected_release() {
        let valid = "https://github.com/yoogoc/zeph/releases/download/v0.2.8/zeph.AppImage";
        assert!(validate_asset_url(&Url::parse(valid).unwrap(), "v0.2.8").is_ok());
        for url in [
            valid.replace("https:", "http:"),
            valid.replace("github.com", "example.com"),
            valid.replace("v0.2.8", "v0.2.6"),
            format!("{valid}?token=x"),
        ] {
            assert!(validate_asset_url(&Url::parse(&url).unwrap(), "v0.2.8").is_err());
        }
    }
    #[test]
    fn missing_keys_and_invalid_signatures_fail_closed() {
        assert!(public_key("").is_err());
        assert!(signature("invalid").is_err());
        let key =
            PublicKey::from_base64("RWQf6LRCGA9i53mlYecO4IzT51TGPpvWucNSCh1CBM0QTaLn73Y7GFO3")
                .unwrap();
        assert!(verify(&key, b"tampered", "aW52YWxpZA==").is_err());
    }
}
