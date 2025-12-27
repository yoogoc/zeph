# Requirements Document

## Introduction

重构WorkspaceSidebar组件，实现树形缩进显示和右键菜单功能，提升用户体验和操作便利性。

## Glossary

- **WorkspaceSidebar**: 工作区侧边栏组件，显示服务、文件夹和模式的树形结构
- **TreeItem**: 树形结构中的单个项目（服务、文件夹或模式）
- **ContextMenu**: 右键点击时显示的上下文菜单
- **TreeIndentation**: 树形结构的缩进显示，用于表示层级关系
- **Service**: HTTP、gRPC或WebSocket服务项目
- **Schema**: 数据模式定义项目
- **Folder**: 用于组织服务或模式的文件夹

## Requirements

### Requirement 1: 树形缩进显示

**User Story:** 作为用户，我希望在侧边栏中看到清晰的树形缩进显示，以便更好地理解项目的层级结构。

#### Acceptance Criteria

1. WHEN 显示文件夹和子项目时，THE WorkspaceSidebar SHALL 根据层级深度显示不同的缩进
2. WHEN 项目嵌套层级增加时，THE WorkspaceSidebar SHALL 按每层16px的间距增加左侧缩进
3. WHEN 文件夹展开或折叠时，THE WorkspaceSidebar SHALL 显示相应的展开/折叠图标
4. WHEN 渲染树形结构时，THE WorkspaceSidebar SHALL 保持一致的视觉层级关系

### Requirement 2: 右键菜单按钮

**User Story:** 作为用户，我希望每个项目右侧都有菜单按钮，以便快速访问相关操作。

#### Acceptance Criteria

1. WHEN 鼠标悬停在任何树形项目上时，THE WorkspaceSidebar SHALL 在右侧显示菜单按钮
2. WHEN 点击菜单按钮时，THE WorkspaceSidebar SHALL 显示对应的下拉菜单
3. WHEN 菜单按钮不活跃时，THE WorkspaceSidebar SHALL 隐藏菜单按钮以保持界面整洁
4. THE WorkspaceSidebar SHALL 为每种项目类型提供不同的菜单按钮样式

### Requirement 3: 文件夹菜单功能

**User Story:** 作为用户，我希望文件夹的右键菜单包含创建、导入和删除选项，以便管理文件夹内容。

#### Acceptance Criteria

1. WHEN 点击文件夹的菜单按钮时，THE WorkspaceSidebar SHALL 显示包含"创建"、"导入"和"删除"选项的菜单
2. WHEN 选择"创建"选项时，THE WorkspaceSidebar SHALL 显示子菜单，包含"创建服务"、"创建模式"和"创建文件夹"选项
3. WHEN 选择"导入"选项时，THE WorkspaceSidebar SHALL 触发导入OpenAPI的对话框
4. WHEN 选择"删除"选项时，THE WorkspaceSidebar SHALL 触发删除文件夹的确认操作

### Requirement 4: 服务和模式菜单功能

**User Story:** 作为用户，我希望服务和模式项目的右键菜单包含删除选项，以便管理这些项目。

#### Acceptance Criteria

1. WHEN 点击服务项目的菜单按钮时，THE WorkspaceSidebar SHALL 显示包含"删除"选项的菜单
2. WHEN 点击模式项目的菜单按钮时，THE WorkspaceSidebar SHALL 显示包含"删除"选项的菜单
3. WHEN 选择删除选项时，THE WorkspaceSidebar SHALL 触发相应的删除确认操作
4. THE WorkspaceSidebar SHALL 为服务和模式提供一致的菜单交互体验

### Requirement 5: 菜单事件处理

**User Story:** 作为开发者，我希望菜单操作能够正确触发相应的事件，以便上层组件能够处理这些操作。

#### Acceptance Criteria

1. WHEN 用户选择创建操作时，THE WorkspaceSidebar SHALL 发出相应的创建事件，包含项目类型和父文件夹信息
2. WHEN 用户选择删除操作时，THE WorkspaceSidebar SHALL 发出相应的删除事件，包含要删除的项目ID
3. WHEN 用户选择导入操作时，THE WorkspaceSidebar SHALL 发出导入事件，包含目标文件夹信息
4. THE WorkspaceSidebar SHALL 确保所有菜单事件都包含必要的上下文信息

### Requirement 6: 视觉交互反馈

**User Story:** 作为用户，我希望在与树形项目交互时获得清晰的视觉反馈，以便了解当前的操作状态。

#### Acceptance Criteria

1. WHEN 鼠标悬停在树形项目上时，THE WorkspaceSidebar SHALL 显示悬停状态的视觉反馈
2. WHEN 项目处于活跃状态时，THE WorkspaceSidebar SHALL 显示不同的背景色或边框样式
3. WHEN 菜单按钮被点击时，THE WorkspaceSidebar SHALL 显示按钮的按下状态
4. THE WorkspaceSidebar SHALL 保持一致的交互反馈样式规范

### Requirement 7: 响应式布局适配

**User Story:** 作为用户，我希望树形结构能够适应不同的侧边栏宽度，以便在各种界面尺寸下都能正常使用。

#### Acceptance Criteria

1. WHEN 侧边栏宽度改变时，THE WorkspaceSidebar SHALL 自动调整项目名称的显示宽度
2. WHEN 项目名称过长时，THE WorkspaceSidebar SHALL 使用省略号截断显示
3. WHEN 缩进层级过深时，THE WorkspaceSidebar SHALL 确保菜单按钮仍然可见和可点击
4. THE WorkspaceSidebar SHALL 在最小宽度下仍然保持基本的可用性