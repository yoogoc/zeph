# Requirements Document

## Introduction

本文档定义了 Zeph API 测试工具中 Service 和 Folder 的重命名和保存修改功能的需求。当前系统虽然有底层的数据更新能力，但缺少用户界面层面的编辑和保存功能，导致用户无法持久化对 Service 和 Folder 的修改。

## Glossary

- **Service**: API 服务实体，包含 HTTP 请求的配置信息（URL、method、headers、body 等）
- **Folder**: 文件夹实体，用于组织 Service，包含共享配置（common headers、search parameters）
- **ServiceStore**: Service 的数据访问层，负责 Service 的 CRUD 操作
- **FolderStore**: Folder 的数据访问层，负责 Folder 的 CRUD 操作
- **HttpService**: Service 详情页面的 UI 组件，显示 HTTP 请求配置界面
- **FolderDetailPage**: Folder 详情页面的 UI 组件，显示 folder 的详细信息
- **Sidebar**: 工作区侧边栏，显示 workspace、folder 和 service 的树形结构
- **PopupMenu**: 右键菜单组件，提供上下文操作选项

## Requirements

### Requirement 1: Service 重命名

**User Story:** 作为 API 测试人员，我希望能够重命名 Service，以便更好地组织和识别我的 API 请求。

#### Acceptance Criteria

1. WHEN 用户在 Sidebar 中右键点击 Service THEN THE System SHALL 在弹出菜单中显示 "Rename" 选项
2. WHEN 用户选择 "Rename" 选项 THEN THE System SHALL 显示重命名对话框，预填充当前 Service 名称
3. WHEN 用户在重命名对话框中输入新名称并确认 THEN THE System SHALL 更新 Service 名称到数据库
4. WHEN Service 名称更新成功 THEN THE System SHALL 在 Sidebar 和所有打开的 Tab 中立即显示新名称
5. WHEN 用户输入空名称或纯空格名称 THEN THE System SHALL 拒绝保存并显示错误提示
6. WHEN 用户点击已聚焦的 Service Tab 标题 THEN THE System SHALL 将 Tab 标题切换为可编辑状态
7. WHEN Tab 标题处于可编辑状态且用户修改后按 Enter 或失去焦点 THEN THE System SHALL 保存新名称到数据库
8. WHEN Tab 标题处于可编辑状态且用户按 Escape THEN THE System SHALL 取消编辑并恢复原名称

### Requirement 2: Folder 重命名

**User Story:** 作为 API 测试人员，我希望能够重命名 Folder，以便更好地组织我的 API 服务分组。

#### Acceptance Criteria

1. WHEN 用户在 Sidebar 中右键点击 Folder THEN THE System SHALL 在弹出菜单中显示 "Rename" 选项
2. WHEN 用户选择 "Rename" 选项 THEN THE System SHALL 显示重命名对话框，预填充当前 Folder 名称
3. WHEN 用户在重命名对话框中输入新名称并确认 THEN THE System SHALL 更新 Folder 名称到数据库
4. WHEN Folder 名称更新成功 THEN THE System SHALL 在 Sidebar 中立即显示新名称
5. WHEN 用户输入空名称或纯空格名称 THEN THE System SHALL 拒绝保存并显示错误提示

### Requirement 3: Service 详情保存

**User Story:** 作为 API 测试人员，我希望能够保存对 Service 配置的修改（URL、method、headers、body 等），以便下次打开时能够继续使用这些配置。

#### Acceptance Criteria

1. WHEN 用户在 HttpService 页面修改任何配置（URL、method、headers、body、query parameters）THEN THE System SHALL 标记该 Service 为已修改状态
2. WHEN Service 处于已修改状态 THEN THE System SHALL 在界面上显示保存按钮
3. WHEN 用户点击保存按钮 THEN THE System SHALL 将所有修改持久化到数据库
4. WHEN 保存成功 THEN THE System SHALL 清除已修改状态标记并隐藏保存按钮
5. WHEN 用户关闭未保存的 Service 页面 THEN THE System SHALL 提示用户是否保存修改
6. WHEN 保存失败 THEN THE System SHALL 显示错误信息并保持已修改状态

### Requirement 4: Folder 基本信息编辑

**User Story:** 作为 API 测试人员，我希望能够在 Folder 详情页面编辑 Folder 的基本信息（名称、描述），以便更新 Folder 的元数据。

#### Acceptance Criteria

1. WHEN 用户打开 FolderDetailPage 的 Overview 标签 THEN THE System SHALL 显示可编辑的名称和描述字段
2. WHEN 用户修改名称或描述字段 THEN THE System SHALL 启用保存按钮
3. WHEN 用户点击保存按钮 THEN THE System SHALL 更新 Folder 的名称和描述到数据库
4. WHEN 保存成功 THEN THE System SHALL 在 Sidebar 中更新 Folder 显示名称
5. WHEN 用户输入空名称或纯空格名称 THEN THE System SHALL 拒绝保存并显示错误提示

### Requirement 5: FolderStore 更新方法

**User Story:** 作为开发者，我需要 FolderStore 提供通用的更新方法，以便能够更新 Folder 的所有字段。

#### Acceptance Criteria

1. THE FolderStore SHALL 提供 `update` 方法，接受 folder_id 和更新字段作为参数
2. WHEN 调用 `update` 方法 THEN THE FolderStore SHALL 更新数据库中对应的 Folder 记录
3. WHEN 更新成功 THEN THE FolderStore SHALL 返回更新后的 Folder 对象
4. WHEN 更新失败（如 folder_id 不存在）THEN THE FolderStore SHALL 返回错误信息
5. THE `update` 方法 SHALL 支持更新 name、description、parent_id 等字段

### Requirement 6: 数据验证

**User Story:** 作为系统，我需要验证用户输入的数据，以确保数据的完整性和一致性。

#### Acceptance Criteria

1. WHEN 用户输入 Service 或 Folder 名称 THEN THE System SHALL 验证名称不为空且不是纯空格
2. WHEN 用户输入的名称长度超过 255 个字符 THEN THE System SHALL 拒绝保存并显示错误提示
3. WHEN 用户输入的描述长度超过 1000 个字符 THEN THE System SHALL 拒绝保存并显示错误提示
4. WHEN 验证失败 THEN THE System SHALL 在输入框附近显示具体的错误信息

### Requirement 7: 国际化支持

**User Story:** 作为国际用户，我希望所有新增的 UI 文本都支持多语言，以便使用我的母语操作应用。

#### Acceptance Criteria

1. THE System SHALL 为所有新增的 UI 文本提供中文（zh-CN）翻译
2. THE System SHALL 为所有新增的 UI 文本提供英文（en）翻译
3. WHEN 用户切换语言 THEN THE System SHALL 立即更新所有 UI 文本为对应语言
4. THE System SHALL 在 `locales/` 目录中维护翻译文件

### Requirement 8: UI 状态管理

**User Story:** 作为系统，我需要正确管理 UI 状态，以确保用户界面的响应性和一致性。

#### Acceptance Criteria

1. WHEN Service 或 Folder 数据更新 THEN THE System SHALL 通知所有相关的 UI 组件刷新显示
2. WHEN 用户在多个标签页中打开同一个 Service THEN THE System SHALL 在所有标签页中同步显示更新
3. WHEN 保存操作进行中 THEN THE System SHALL 禁用保存按钮并显示加载状态
4. WHEN 保存操作完成 THEN THE System SHALL 恢复按钮状态并显示操作结果

