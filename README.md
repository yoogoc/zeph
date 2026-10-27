# Zeph

用 Rust + gpui-kit 实现的本地 API 工作台。无需账号，没有云端同步、遥测或浏览器 CORS 限制。界面与请求执行均为原生实现。

## 运行

```sh
cargo run
# 指定本地项目目录；目录不存在时初始化示例集合
cargo run -- --project /path/to/project
```

工作区是应用内的逻辑容器，拥有稳定 ID，独立管理请求、环境、接口定义与历史。点击标题栏的“工作区”标签进入独立管理页，输入名称创建，或打开已有工作区。关闭标签后工作区仍在列表中；重命名保持身份、草稿和历史。

应用自动管理本地存储。macOS 工作区列表保存在 `~/Library/Application Support/Zeph/workspaces.json`，新建工作区的数据保存在其下的 `workspaces/<id>/`。旧版项目和 `session.json` 会自动登记为逻辑工作区，保留原数据文件。`--project` 是兼容旧项目 / 开发测试的入口，使用项目内独立的列表，不改全局工作区列表。macOS 已完成实际窗口联调；其他桌面平台尚未实测。当前使用 gpui-kit 0.7.1，构建不需要系统 protoc。

开启本地演示服务，然后在另一个终端启动 Zeph：

```sh
cargo run --example local_server --no-default-features
cargo run
```

演示服务仅监听 loopback：HTTP / WS `127.0.0.1:8080`，gRPC + Reflection `127.0.0.1:50051`。数据为临时演示数据，退出后丢弃。CONNECT 需要另行配置 HTTP 代理。

macOS 可生成开发用 App 包：

```sh
./scripts/bundle-macos.sh
open target/debug/Zeph.app
```

## 第一版已实现

- 工作区：标题栏内切换、关闭多个工作区，提供独立的工作区管理页，在“工作区与环境”中编辑名称；默认启动恢复上次打开的工作区；从应用内列表打开；重复打开同一工作区会切换到已有标签。
- 请求：每个工作区可同时打开多个请求标签，切换时保留草稿与面板；关闭标签保留请求和运行中的会话；支持重命名、复制、上移 / 下移、删除及本次运行内撤销删除。集合行的 `⋯` 或右键打开操作菜单。请求标签长名称自动省略；标签较多时支持横向滚动、自动定位当前请求，以及右侧列表切换。
- 工作台：请求搜索、HTTP / gRPC / WS 新建、上下 / 左右布局；侧栏、Schema 与请求 / 响应分隔条可拖动，尺寸按工作区保存；Schema 面板、浅色 / 深色、本地历史、接口定义原文与导入诊断。
- 环境：新建、复制、重命名、删除与恢复上次删除；变量草稿使用“应用变量”保存，至少保留一个环境。
- 预览：展开后的 URL、Query / Path / Headers 与 Body 编码摘要、JSON 格式化、响应复制与保存预览；HTTP 实测时间线、WS 握手、gRPC Metadata 与 Trailers 分开展示。multipart 展示字段清单，文件内容及 boundary 在实际发送时生成；编码摘要隐藏认证 Header。
- HTTP：GET、POST、PUT、PATCH、DELETE、HEAD、OPTIONS、TRACE、QUERY，也可直接输入自定义 Method；CONNECT 为真实代理隧道握手探测，成功后关闭隧道。
- Query、Path、Headers：逐项编辑、启用、删除；保留重复 Query / Header / 表单键。Path 值作为单个路径段编码。
- Body：None、JSON、multipart/form-data（文本和文件）、x-www-form-urlencoded、Text、XML、Binary。Body 不与 method 硬绑定。
- 认证 / 设置：None、Bearer、Basic；HTTP 超时、取消、代理、重定向、TLS 校验。TRACE 拒绝携带 Body、Authorization 或 Cookie。
- gRPC：导入 `.proto` / descriptor set；Reflection v1 / v1alpha；搜索并选择 `package.Service/Method`；根据方法生成 ProtoJSON 模板；Unary、服务端流、客户端流、双向流；发送、Half-close、取消、Metadata / Trailers。
- WS：ws / wss 连接、Headers / Auth、Text / Binary 文件发送、消息和 Ping / Pong 记录、关闭、取消。
- OpenAPI：Swagger 2.0、OpenAPI 3.0.x、3.1.x 的 JSON / YAML 导入；生成 HTTP 请求、参数与 Body 示例；本地 JSON Pointer / 文件引用；保留原文、读取的依赖文档和导入诊断。

左侧顶部“＋”菜单统一新建 HTTP、gRPC 和 WebSocket 请求。标题栏保留环境选择与“变量”；主题、保存、编码预览和复制实际 URL 收进“更多”。布局切换位于响应区工具栏。

地址栏平时直接显示实际发送地址：环境变量紫色、Path 值青色、Query 值金色；点击地址栏编辑原始模板，Enter 或失焦后回到实际地址，保留变量绑定。Query 参数仍在参数表中独立编辑，避免展开后的地址被再次保存并重复追加参数。

gRPC / WS 响应以收发消息列表与正文查看器展示，显示方向、顺序、相对时间与载荷大小，JSON 自动格式化；Metadata、Trailers、连接事件和错误分别保留在对应标签与时间线。

发送 `⌘ Enter`，保存 `⌘ S`，查找请求 `⌘ K`，新建 HTTP 请求 `⌘ N`，关闭当前标签 `⌘ W`，撤销删除 `⌘ Shift Z`。环境变量使用 `{{name}}`，支持嵌套变量，缺失或循环变量会阻止发送。内置集合里的响应不是预置数据；只有实际发起请求后才显示响应。

## 文件与模块

```text
zeph.project.json          请求、打开的标签、环境变量、布局；500 ms 防抖原子保存
contracts/<id>/
  original.json            原始 OpenAPI 文档内容
  sources.json             已读取的本地依赖、来源与诊断
.zeph/descriptors/         gRPC 已编译 / Reflection 读取的 descriptors
.zeph/history/             实际执行结果，每次一个 JSON 文件
```

项目和历史文件使用 Unix `0600` 权限；认证及环境变量目前保存在本地 JSON 中，尚未接入系统钥匙串。历史保留响应内容，HTTP `Set-Cookie` 隐藏。项目文件损坏时启动报错，保留原文件。

`model` 管理可持久化的数据；`storage` 负责原子写入与防止旧保存覆盖新修改；`openapi` 负责版本适配和离线引用；`transport` 负责三个协议；`ui` 只通过有界消息队列接收后台 Tokio 工作结果。不同请求及工作区可并行执行，每个请求同时保留一个运行实例。切换标签或工作区不会重绑任务；删除请求或关闭工作区会取消对应任务。已删除请求的执行历史仍可查看。

## OpenAPI 的实现决策

先实现自己的**版本适配与导入层**，不从零重写完整 OpenAPI / JSON Schema 引擎。现有代码以无损 `serde_json::Value` 保留契约，再按各版本规则投影到可执行请求模型。导入成功不等于完成规范验证，也不意味着整个规范已经完整支持。

本版本不执行完整 JSON Schema 校验。复杂数组 / 对象参数序列化、multipart encoding、`$ref` 兄弟语义、boolean Schema、动态引用 / anchors / dialect、自定义安全方案、callbacks / webhooks 等会保留并输出诊断；Cookie 参数需手动配置。网络 `$ref` 不自动下载。示例生成选择一个分支，并有深度上限；生成示例不保证满足所有约束。Swagger 2.0 自定义 consumes / produces 和复杂组合需检查。多种 Body 媒体类型默认选 JSON 或第一种，其他类型需手动切换。

下一阶段应补规范测试集、完整参数序列化、响应 Schema 校验和契约编辑 / 导出，再评估接入 JSON Schema 验证库。不要把当前 importer 当成完整 OpenAPI 库对外发布。

## 当前边界

HTTP 响应预览最多 8 MiB，流会在超限后停止；WS 接收消息上限 8 MiB，单条会话消息预览上限 64 KiB，会话日志最多约 1 MiB。尚未实现完整响应下载、二进制响应查看、自动重连、请求脚本、OAuth、mTLS、自定义 CA、Cookie jar、系统钥匙串和契约导出。gRPC / WS 当前使用系统证书校验，代理和关闭校验会明确报错。CONNECT 是隧道握手探测，不是持久代理终端。

`.proto` 的 include 根目录目前是所选文件的父目录；跨目录 import 可先生成包含 dependencies 的 descriptor set 再导入。

## 验证

```sh
cargo fmt --check
cargo test --no-default-features
cargo test --lib
cargo clippy --all-targets
cargo build
```

测试使用临时项目与 loopback 服务：工作区与历史隔离、旧项目迁移、多请求标签恢复、删除与空集合、逻辑工作区 ID / 列表恢复 / 旧数据迁移、编码预览、Metadata / Trailers 与消息分离、收发顺序与预览上限，以及 10 种 HTTP method（含 QUERY）、重复参数、全部 Body 编码、取消、WS echo、OpenAPI 三个版本 / 本地引用、存储原子性、ProtoJSON 64-bit、gRPC Reflection 和四种调用模式，以及 gRPC 错误状态。原生窗口已实际验证请求发送与方法选择。
