# Zeph 发版

`src/updater/` 的更新客户端与安装助手改编自同作者的 beacon-updater。Zeph 使用独立的 Minisign 密钥，不共用 Beacon 的信任链。

## 签名

- 客户端嵌入 `assets/packaging/update-public-key`（Minisign 公钥文本的 base64）。
- 私钥保存在仓库 Secret `ZEPH_UPDATE_PRIVATE_KEY`。本机备份位于 `~/.config/zeph/release-signing/minisign.key`，权限为 `0600`；不要提交私钥。
- 安装包与 `update.json` 都独立签名。先上传到草稿 Release，客户端验签工具通过后才发布，避免暴露不完整的更新。
- `.sig` 文件内容为 Minisign 签名文本的 base64，与客户端验签格式一致。

## 发布步骤

1. 在 `main` 分支修改 `Cargo.toml` 的版本（以及 lockfile 中根包的版本），提交并推送。正式版采用 `X.Y.Z`，开发版采用 `X.Y.Z-dev.N`。
2. 在同一提交创建并推送 `vX.Y.Z` 或 `vX.Y.Z-dev.N` 标签。标签版本必须与编译时版本一致。
3. `Signed multi-platform release` 工作流在六种原生 runner 上分别测试、构建和打包，正式版本要求所有任务成功。
4. 发布任务使用 Secret 给所有安装包、SHA256SUMS 与更新清单签名，再运行 Rust 客户端的 `verify-update` 验签。成功后发布草稿；开发版本标记为 prerelease。

也可手动运行工作流并指定一个已存在的标签。已公开的 Release 不允许被覆盖；需修复时发布新版本。

## 分发格式

与 Beacon 一样，提供原生安装包，直接在 GitHub Release 附件下载：

| 系统 | 架构 | 用户下载安装 | 自动更新 |
| --- | --- | --- | --- |
| macOS | Apple Silicon / Intel | `.dmg`，打开后拖入 Applications | `.app.tar.gz` |
| Windows | x64 / ARM64 | `-setup.exe`（NSIS） | 已安装的 NSIS 安装器 |
| Linux | x64 / ARM64 | `.deb` / `.AppImage` | AppImage 原位替换；DEB 手动安装 |

包名统一为 `Zeph_<version>_<system>-<arch>` 加格式后缀。系统名为 `macos`、`windows`、`linux`，架构为 `aarch64` 或 `x86_64`。同一版本总计 10 个分发文件，另附各文件的 Minisign `.sig`、签名 `SHA256SUMS` 和签名 `update.json`。缺少任一分发文件会阻止发布；既有公开版本不会被替换。

更新平台键为 `macos-<arch>-app`、`windows-<arch>-nsis`、`linux-<arch>-appimage`、`linux-<arch>-deb`，与客户端安装类型识别一致。macOS DMG 和自动更新归档包含同一份带应用图标的 ad-hoc 签名 `.app`。

Windows 静态链接 VC++ 运行时，应用图标嵌入 exe；Linux DEB 声明图形、字体、DBus 等运行依赖。Linux 包基于 Ubuntu 24.04 构建，AppImage 仍需系统提供兼容的 glibc 和图形驱动，并不代表所有发行版都已验证。

本地开发构建仍使用 `./scripts/bundle-macos.sh`。正式分发打包先 `cargo build --locked --release --bin zeph`，再按当前机器系统及架构运行（Windows 的 binary 路径为 `target/release/zeph.exe`）：

```sh
# macOS
python3 scripts/release.py native --version 0.0.3 --platform macos-aarch64 --binary target/release/zeph --output dist
# Linux / Windows 先安装打包工具
cargo install cargo-packager --version 0.11.8 --locked
python3 scripts/release.py native --version 0.0.3 --platform linux-x86_64 --binary target/release/zeph --output dist
```

Linux / Windows 使用 [cargo-packager](https://docs.crabnebula.dev/packager/)，配置在 `Cargo.toml` 的 `package.metadata.packager`；macOS 使用系统 `codesign` / `hdiutil` 打包。图标源文件为 `assets/app-icon/zeph.svg`，对应 PNG、ICO、ICNS 与源码一同提交。各平台构建的成功状态以发布工作流为准，不能用本机 macOS 的验证代替 Windows / Linux 原生检查。

Minisign 用于应用更新验签，不等于 Apple Developer ID 签名或公证。当前归档中的应用采用 ad-hoc codesign；首次通过浏览器下载时仍可能需要用户在 macOS 中允许打开。

## 验证

```sh
cargo test --all-targets
cargo clippy --all-targets -- -D warnings
python3 -m unittest discover -s scripts/tests
ZEPH_UPDATE_PUBLIC_KEY="$(cat assets/packaging/update-public-key)" cargo run --no-default-features --bin verify-update -- artifacts
```

安装的替换 / 恢复逻辑在临时目录中测试，下载采用 loopback 服务验证正常签名、篡改、截断与取消。不会为了测试覆盖正在使用的应用。
