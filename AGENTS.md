# 构建与缓存

本机的 `cargo` 已接入 `mbx`，构建、检查、测试、格式化和运行通过现有 `cargo` 命令统一交给 mbx 管理。

- 保留脚本和文档中的 `cargo` 命令，无需改写为显式的 `mbx` 命令；不要绕过现有接管配置。
- macOS 打包使用 `./scripts/bundle-macos.sh`，脚本内部保留 `cargo build`。
- 清理本项目构建产物和增量状态使用 `mbx clean`；保留 mbx 的共享编译缓存和 Cargo 下载的依赖。
