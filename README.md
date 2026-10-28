# Zeph

用 Rust + gpui-kit 实现的本地 API 工作台。无需账号，没有云端同步、遥测或浏览器 CORS 限制。界面与请求执行均为原生实现。

## 运行

本机 `cargo` 已接入 mbx，继续使用常规 `cargo` 命令，由 mbx 管理共享编译缓存和构建目录。清理本项目的构建产物与增量状态使用 `mbx clean`。

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

## 应用设置与更新

标题栏“设置”、应用菜单 Settings… 或 `⌘ ,`（Windows/Linux 为 `Ctrl ,`）打开应用级配置页；设置在所有逻辑工作区之间共享。支持跟随系统 / 浅色 / 深色主题、新请求的超时、TLS 校验与代理默认值，以及应用更新偏好。修改默认值不会覆盖已有请求。

配置保存在应用数据目录的 `settings.json`；使用 `--project` 开发入口时，配置隔离在该目录的 `.zeph/application/settings.json`，不修改日常配置。

更新从公开的 [yoogoc/zeph Releases](https://github.com/yoogoc/zeph/releases) 获取，无需 Token。默认启动时检查一次，此后每小时检查；自动下载默认关闭。稳定版与开发版通道分别选择，不会降级。元数据和下载均有大小 / 超时限制，清单和安装包均通过 Minisign 验签；取消或下载失败会清理临时文件。自动检查失败不会打断调试。

macOS `.app` 可下载、验证并在确认后重启安装；安装前保存所有工作区，更新助手等待旧进程退出、再次验签，再替换应用，替换失败时恢复原应用。发布流水线提供 macOS DMG、Windows NSIS 安装器，以及 Linux DEB / AppImage，各自区分 x64 与 ARM64；源码或包管理器安装使用 Releases 下载入口。签名密钥与公钥配置见 [发布说明](docs/RELEASING.md)。Windows 安装版与 Linux AppImage 支持签名更新；DEB 安装通过下载新版软件包更新。

## 第一版已实现

- 工作区：标题栏内切换、关闭多个工作区，提供独立的工作区管理页，在“工作区与环境”中编辑名称；默认启动恢复上次打开的工作区；从应用内列表打开；重复打开同一工作区会切换到已有标签。
- 文件夹：工作区内的逻辑分组，支持嵌套、新建、重命名、折叠、移动请求和文件夹；删除仅移除分组，内容移至上一级并可撤销。搜索自动展开匹配路径，新建请求进入选中的文件夹。
- 请求：每个工作区可同时打开多个请求标签，切换时保留草稿与面板；关闭标签保留请求和运行中的会话；支持重命名、复制、上移 / 下移、删除及本次运行内撤销删除。集合行的 `⋯` 或右键打开操作菜单。请求标签长名称自动省略；标签较多时支持横向滚动、自动定位当前请求，以及右侧列表切换。
- 工作台：请求搜索、HTTP / gRPC / WS 新建、上下 / 左右布局；侧栏、Schema 与请求 / 响应分隔条可拖动，尺寸按工作区保存；Schema 面板、浅色 / 深色、本地历史、接口定义原文与导入诊断。
- 环境：新建、复制、重命名、删除与恢复上次删除；变量草稿使用“应用变量”保存，至少保留一个环境。
- 预览：展开后的 URL、Query / Path / Headers 与 Body 编码摘要、JSON 格式化、响应复制与保存预览；HTTP 实测时间线、WS 握手、gRPC Metadata 与 Trailers 分开展示。multipart 展示字段清单，文件内容及 boundary 在实际发送时生成；编码摘要隐藏认证 Header。
- HTTP：GET、POST、PUT、PATCH、DELETE、HEAD、OPTIONS、TRACE、QUERY，也可直接输入自定义 Method；CONNECT 为真实代理隧道握手探测，成功后关闭隧道。
- Query、Path、Headers：逐项编辑、启用、删除；保留重复 Query / Header / 表单键。Path 值作为单个路径段编码。
- Body：None、JSON、multipart/form-data（文本和文件）、x-www-form-urlencoded、Text、XML、Binary。Body 不与 method 硬绑定。
- 认证 / 设置：继承上级、无认证、Bearer、Basic；HTTP 超时、取消、代理、重定向、TLS 校验。TRACE 拒绝携带 Body、Authorization 或 Cookie。
- gRPC：导入 `.proto` / descriptor set；Reflection v1 / v1alpha；搜索并选择 `package.Service/Method`；根据方法生成 ProtoJSON 模板；Unary、服务端流、客户端流、双向流；发送、Half-close、取消、Metadata / Trailers。
- WS：ws / wss 连接、Headers / Auth、Text / Binary 文件发送、消息和 Ping / Pong 记录、关闭、取消。
- OpenAPI：Swagger 2.0、OpenAPI 3.0.x、3.1.x 的 JSON / YAML 导入；每份文档建立独立根文件夹，按第一个 operation tag 分组，其余 tags 保留在映射中；无 tag 的接口放在文档根目录；生成 HTTP 请求、参数与 Body 示例；本地 JSON Pointer / 文件引用；保留原文、读取的依赖文档和导入诊断。
- HTTP 接口结构：以字段卡片和可折叠结构树展示参数、Body、响应与模型，包含类型、必填、说明、可选值、默认值和约束。Query / Path / Header / 表单的 Value 输入框点入即显示枚举选项，输入时筛选，支持上下键选择、Enter 确认和 Esc 收起，也保留自由输入和 `{{variable}}`；选值只修改本地调试值。结构读取已应用的 Apifox 定义或保存的 OpenAPI 快照，原文仍可在“接口定义”查看。
- Apifox 数据源：直连官方导出 API，单向拉取 HTTP 定义、Schema、示例和嵌套目录；支持多个连接、变更预览、逐字段冲突处理、手动 / 定时检查、自动应用无冲突更新和整批撤销。

左侧顶部“＋”菜单统一新建 HTTP、gRPC 和 WebSocket 请求。标题栏保留环境选择与“变量”；主题、保存、编码预览和复制实际 URL 收进“更多”。布局切换位于响应区工具栏。

地址栏平时直接显示实际发送地址：环境变量紫色、Path 值青色、Query 值金色；点击地址栏编辑原始模板，Enter 或失焦后回到实际地址，保留变量绑定。Query 参数仍在参数表中独立编辑，避免展开后的地址被再次保存并重复追加参数。

## 工作区与文件夹认证

侧栏“工作区设置”或标题栏“更多 → 工作区设置”编辑工作区名称与 Auth。文件夹的 `⋯` / 右键菜单中选择“文件夹设置”，可配置该目录的名称与 Auth；这些设置属于逻辑工作区，不是磁盘目录设置，也不属于应用级设置。工作区和各文件夹的设置会作为独立标签与请求并列打开，重复打开会定位已有标签；切换标签保留未保存的编辑内容。设置标签中 `⌘ S` 保存当前设置，`⌘ W` 关闭当前标签。

请求的 Auth 默认“继承上级”：从所在文件夹逐层向上查找最近的配置，最后使用工作区 Auth。文件夹也可继承上级；任一级选择“无认证”即停止查找，请求仍可使用自己的 Bearer / Basic 覆盖。Auth 页显示实际来源，并提供“编辑来源设置”入口，不展示继承的明文凭据。

HTTP、gRPC 调用与 Reflection、WebSocket 握手共用相同的解析规则，支持 `{{variable}}` 并使用发送时的当前环境。认证只在临时执行快照中展开，不写回接口；移动或删除文件夹、更改上级设置后，下次发送自动重新解析，已运行的连接不变。编码预览隐藏认证值。

旧项目中已有 Bearer / Basic 保持独立配置，未配置认证的请求可直接继承。明确选择“无认证”会持久保存这一覆盖策略。OpenAPI 声明 `security: []` 的公开操作导入后默认不继承；用户仍可在 Auth 页主动更改。Apifox 定义同步不会覆盖本地工作区、文件夹或请求的认证设置。

凭据仍按工作区本地数据保存，建议通过环境变量引用；继承不会在子接口中重复保存凭据。

gRPC / WS 响应以收发消息列表与正文查看器展示，显示方向、顺序、相对时间与载荷大小，JSON 自动格式化；Metadata、Trailers、连接事件和错误分别保留在对应标签与时间线。

发送 `⌘ Enter`，保存 `⌘ S`，查找请求 `⌘ K`，新建 HTTP 请求 `⌘ N`，关闭当前标签 `⌘ W`，撤销删除 `⌘ Shift Z`。环境变量使用 `{{name}}`，支持嵌套变量，缺失或循环变量会阻止发送。内置集合里的响应不是预置数据；只有实际发起请求后才显示响应。

文件夹菜单与请求菜单中的“查看 OpenAPI 映射”可定位保存的契约快照，查看原始 tag、path、method、operationId 与文档来源。移动和重命名本地分组保留这些来源信息，当前不会自动改写源 OpenAPI 文件。同名文档分别保存，不会合并；tag 中的 `/` 作为完整 tag 名保留，不擅自解释为层级。

旧版已导入请求在存在唯一对应的本地契约快照时自动迁移为分组，保留请求 ID、标签和历史；同一源文件存在多份旧快照时保持原状，重新导入可建立明确映射。

## Apifox 同步

左侧“数据源 → 添加数据源”中填写连接名称、项目 ID 和 API 访问令牌。令牌从 Apifox 的“账号设置 → API 访问令牌”生成；项目 ID 位于项目设置中。可以先“验证连接”，再“保存并预览”，勾选需要的变更后应用。

每个数据源同步一个项目的指定分支与模块，留空时使用主分支和默认模块；多个模块分别建立连接。修改项目、分支、模块或目标文件夹会创建新数据源，避免把范围变化误判成删除。同步目标是工作区内的逻辑文件夹；已关联目标需先断开连接才能删除分组。

使用 `POST https://api.apifox.com/v1/projects/{projectId}/export-openapi`，固定 API 版本 `2024-03-28`，拉取 OpenAPI 3.1 JSON，并启用 Apifox 扩展。`x-apifox-folder` 优先于 tags，多级目录正确处理 `\/`、`\\` 转义；普通 tag 中的 `/` 不拆分。此接口不用于同步 gRPC、WS、完整环境变量、测试场景或脚本，也不向 Apifox 回写。

每次检查拉取完整快照，在本地计算差异。仅应用已选择的资源，跳过项和冲突持续保留；本地改动发生后重新计算预览。优先用唯一 operationId 识别接口，否则按 method + path 匹配；身份无法确认的路径变化会显示为新增与移除，不猜测关联。

现有请求的参数值、Body、认证、上传文件、标签页和历史保留；定义中的新增参数补入草稿，已填写的值不覆盖。移除的本地字段暂时保留，请按更新后的定义检查兼容性。远端移除默认不选中，可保留并标记或移到“已移除”；本地已删除请求不会自动恢复。复制同步请求生成独立的本地请求。

默认仅手动检查。可设置运行期间每 5 / 15 / 30 分钟检查，并选择自动应用无冲突更新；冲突与移除始终人工处理。断网保留上次快照，失败退避重试，令牌失效或无权限时暂停自动检查。最近 10 批应用记录保存撤销信息，按批次倒序撤销；同期调试值继续保留，后续定义编辑冲突时拒绝覆盖。

Apifox 访问令牌保存在系统凭据存储（macOS Keychain / Windows Credential Manager / Linux Secret Service），工作区只记录来源 ID。快照、定义基线、来源绑定和撤销记录随工作区原子保存；不记录 Authorization 或 API 错误响应正文。Linux 构建需提供 Secret Service 的 D-Bus 开发依赖。

## 文件与模块

```text
zeph.project.json          请求、标签、环境、布局、数据源、快照与同步基线；500 ms 防抖原子保存
contracts/<id>/
  original.json            原始 OpenAPI 文档内容
  sources.json             已读取的本地依赖、来源与诊断
.zeph/descriptors/         gRPC 已编译 / Reflection 读取的 descriptors
.zeph/history/             实际执行结果，每次一个 JSON 文件
```

项目和历史文件使用 Unix `0600` 权限；请求自身的认证及环境变量目前保存在本地 JSON 中，Apifox 数据源令牌单独存入系统凭据存储。历史保留响应内容，HTTP `Set-Cookie` 隐藏。项目文件损坏时启动报错，保留原文件。

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
