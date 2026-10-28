# Zeph 发版

`src/updater/` 的更新客户端与安装助手改编自同作者的 beacon-updater。Zeph 使用独立的 Minisign 密钥，不共用 Beacon 的信任链。

## 签名

- 客户端嵌入 `assets/packaging/update-public-key`（Minisign 公钥文本的 base64）。
- 私钥保存在仓库 Secret `ZEPH_UPDATE_PRIVATE_KEY`。本机备份位于 `~/.config/zeph/release-signing/minisign.key`，权限为 `0600`；不要提交私钥。
- 安装包与 `update.json` 都独立签名。先上传到草稿 Release，客户端验签工具通过后才发布，避免暴露不完整的更新。
- `.sig` 文件内容为 Minisign 签名文本的 base64，与客户端验签格式一致。

## 发布步骤

1. 在此 Rust 实现所在分支修改 `Cargo.toml` 的版本（以及 lockfile 中根包的版本），提交并推送。正式版采用 `X.Y.Z`，开发版采用 `X.Y.Z-dev.N`。
2. 在同一提交创建并推送 `vX.Y.Z` 或 `vX.Y.Z-dev.N` 标签。标签版本必须与编译时版本一致。
3. `Signed macOS release` 工作流在 Apple Silicon 和 Intel runner 上分别测试、构建和打包。
4. 发布任务使用 Secret 给两个归档与更新清单签名，再运行 Rust 客户端的 `verify-update` 验签。成功后发布草稿；开发版本标记为 prerelease。

也可手动运行工作流并指定一个已存在的标签。已公开的 Release 不允许被覆盖；需修复时发布新版本。

包名为 `Zeph_<version>_macos-aarch64.app.tar.gz` 和 `Zeph_<version>_macos-x86_64.app.tar.gz`。更新清单的平台键为 `macos-aarch64-app` 与 `macos-x86_64-app`。运行器标签依据 [GitHub 官方列表](https://docs.github.com/en/actions/reference/runners/github-hosted-runners) 选择。

Minisign 用于应用更新验签，不等于 Apple Developer ID 签名或公证。当前归档中的应用采用 ad-hoc codesign；首次通过浏览器下载时仍可能需要用户在 macOS 中允许打开。

## 验证

```sh
cargo test --all-targets
cargo clippy --all-targets -- -D warnings
python3 -m unittest discover -s scripts/tests
ZEPH_UPDATE_PUBLIC_KEY="$(cat assets/packaging/update-public-key)" cargo run --no-default-features --bin verify-update -- artifacts
```

安装的替换 / 恢复逻辑在临时目录中测试，下载采用 loopback 服务验证正常签名、篡改、截断与取消。不会为了测试覆盖正在使用的应用。
