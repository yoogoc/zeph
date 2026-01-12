# Requirements Document

## Introduction

重新实现 OpenAPI 导入功能，简化现有的复杂事件处理机制，支持直接导入到指定文件夹下。参考 `import_dialog.rs` 的设计模式，采用更简洁的事件驱动架构。

## Glossary

- **OpenAPI_Importer**: 负责解析和导入 OpenAPI 规范的系统组件
- **Import_Dialog**: 显示导入选项的对话框界面，参考现有 `import_dialog.rs` 实现
- **Target_Folder**: 用户选择的用于存放导入内容的目标文件夹
- **Service_Item**: 从 OpenAPI 规范导入的 HTTP 服务项目
- **Popup_Menu**: 右键菜单组件，用于触发导入操作

## Requirements

### Requirement 1: 从文件夹菜单触发导入

**User Story:** 作为用户，我希望能够在文件夹的右键菜单中选择"导入 OpenAPI"，以便将服务直接导入到该文件夹下。

#### Acceptance Criteria

1. WHEN 用户右键点击服务类型文件夹 THEN THE Popup_Menu SHALL 显示"导入 OpenAPI"选项
2. WHEN 用户点击"导入 OpenAPI"选项 THEN THE Import_Dialog SHALL 打开并预设目标文件夹为当前文件夹
3. WHEN 用户右键点击服务根节点 THEN THE Popup_Menu SHALL 显示"导入 OpenAPI"选项
4. WHEN 用户从根节点触发导入 THEN THE Import_Dialog SHALL 打开且目标文件夹为空（使用默认行为）

### Requirement 2: 简化的导入对话框

**User Story:** 作为用户，我希望导入对话框简洁明了，只包含必要的输入和选项。

#### Acceptance Criteria

1. THE Import_Dialog SHALL 包含 OpenAPI 内容输入区域（支持 JSON/YAML 内容或 URL）
2. THE Import_Dialog SHALL 显示预设的目标文件夹名称（如果有）
3. THE Import_Dialog SHALL 提供"创建子文件夹"选项开关
4. WHEN 用户输入有效的 OpenAPI 内容 THEN THE Import_Dialog SHALL 显示将要导入的服务数量
5. THE Import_Dialog SHALL 提供"导入"和"取消"两个操作按钮

### Requirement 6: URL 导入支持

**User Story:** 作为用户，我希望能够通过 URL 导入 OpenAPI 规范，以便直接从远程服务器获取最新的 API 定义。

#### Acceptance Criteria

1. WHEN 用户输入以 http:// 或 https:// 开头的内容 THEN THE OpenAPI_Importer SHALL 识别为 URL
2. WHEN 检测到 URL 输入 THEN THE OpenAPI_Importer SHALL 从该 URL 下载 OpenAPI 内容
3. WHEN URL 下载成功 THEN THE OpenAPI_Importer SHALL 使用下载的内容进行导入
4. WHEN URL 下载失败 THEN THE Import_Dialog SHALL 显示网络错误信息
5. WHILE 正在下载 URL 内容 THEN THE Import_Dialog SHALL 显示加载状态

### Requirement 3: 简化的事件处理

**User Story:** 作为开发者，我希望事件处理机制简洁清晰，减少不必要的中间事件。

#### Acceptance Criteria

1. THE Import_Dialog SHALL 只发出两种事件：Import 和 Cancel
2. WHEN 用户点击"导入"按钮 THEN THE Import_Dialog SHALL 发出包含完整配置的 Import 事件
3. WHEN 用户点击"取消"按钮 THEN THE Import_Dialog SHALL 发出 Cancel 事件
4. THE WorkspaceSidebar SHALL 直接处理 Import 事件并调用导入逻辑
5. THE Import_Dialog SHALL 不发出预览请求或冲突解决等中间事件

### Requirement 4: 导入执行

**User Story:** 作为用户，我希望导入操作能够正确地将服务放置到指定的文件夹中。

#### Acceptance Criteria

1. WHEN 用户确认导入且指定了目标文件夹 THEN THE OpenAPI_Importer SHALL 将所有服务导入到该文件夹
2. WHEN 用户启用"创建子文件夹"选项 THEN THE OpenAPI_Importer SHALL 在目标文件夹下根据 tags 创建子文件夹
3. WHEN 用户禁用"创建子文件夹"选项 THEN THE OpenAPI_Importer SHALL 将所有服务直接放入目标文件夹
4. WHEN 导入完成 THEN THE Import_Dialog SHALL 自动关闭
5. WHEN 导入完成 THEN THE WorkspaceSidebar SHALL 刷新显示新导入的服务

### Requirement 5: 错误处理

**User Story:** 作为用户，我希望在导入失败时能够看到清晰的错误信息。

#### Acceptance Criteria

1. WHEN OpenAPI 内容格式无效 THEN THE Import_Dialog SHALL 显示解析错误信息
2. WHEN 目标文件夹不存在 THEN THE OpenAPI_Importer SHALL 显示"目标文件夹不存在"错误
3. WHEN 导入过程中发生错误 THEN THE Import_Dialog SHALL 显示错误详情并保持打开状态
4. THE Import_Dialog SHALL 在错误区域使用醒目的样式显示错误信息
