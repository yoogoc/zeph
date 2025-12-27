# Design Document

## Overview

本设计文档描述了WorkspaceSidebar组件的重构方案，主要目标是实现树形缩进显示和右键菜单功能。重构将保持现有的功能完整性，同时提升用户体验和代码可维护性。

## Architecture

### 组件结构

```
WorkspaceSidebar
├── SearchBar (搜索栏)
├── ServicesSection (服务区域)
│   ├── SectionHeader (区域标题 + 操作按钮)
│   └── TreeView (树形视图)
│       ├── TreeItem (文件夹/服务项目)
│       │   ├── ItemContent (项目内容)
│       │   └── ContextMenuButton (右键菜单按钮)
│       └── ...
└── SchemasSection (模式区域)
    ├── SectionHeader
    └── TreeView
        └── TreeItem
            ├── ItemContent
            └── ContextMenuButton
```

### 数据流

1. **状态管理**: 使用现有的`expanded_folders`和`expanded_sections`状态
2. **事件传播**: 通过`WorkspaceSidebarEvent`向上传递用户操作
3. **菜单交互**: 使用gpui_component的DropdownMenu组件实现上下文菜单

## Components and Interfaces

### TreeItem组件

新增的核心组件，用于渲染树形结构中的单个项目：

```rust
struct TreeItem {
    item_type: TreeItemType,
    depth: usize,
    is_expanded: bool,
    is_active: bool,
    show_menu: bool,
}

enum TreeItemType {
    Folder(Folder),
    Service(Service),
    Schema(Schema),
}
```

### ContextMenuButton组件

右键菜单按钮组件：

```rust
struct ContextMenuButton {
    item_id: Uuid,
    menu_type: MenuType,
    visible: bool,
}

enum MenuType {
    Folder(FolderMenuOptions),
    Service(ServiceMenuOptions),
    Schema(SchemaMenuOptions),
}
```

### 菜单选项定义

```rust
struct FolderMenuOptions {
    can_create_service: bool,
    can_create_schema: bool,
    can_create_folder: bool,
    can_import: bool,
    can_delete: bool,
}

struct ServiceMenuOptions {
    can_delete: bool,
}

struct SchemaMenuOptions {
    can_delete: bool,
}
```

## Data Models

### 扩展现有事件类型

```rust
pub enum WorkspaceSidebarEvent {
    // 现有事件...
    OpenService(Uuid),
    OpenSchema(Uuid),
    CreateService(String, Option<Uuid>),
    CreateSchema(String, Option<Uuid>),
    CreateFolder(String, Option<Uuid>, FolderType),
    ToggleFolder(Uuid),
    ImportOpenApi(ImportConfig),
    
    // 新增菜单事件
    DeleteService(Uuid),
    DeleteSchema(Uuid),
    DeleteFolder(Uuid),
    ShowContextMenu(Uuid, MenuType),
}
```

### 树形项目渲染状态

```rust
struct TreeItemState {
    id: Uuid,
    name: String,
    item_type: TreeItemType,
    depth: usize,
    is_expanded: bool,
    is_active: bool,
    parent_id: Option<Uuid>,
    children_count: usize,
}
```

## Correctness Properties

*A property is a characteristic or behavior that should hold true across all valid executions of a system-essentially, a formal statement about what the system should do. Properties serve as the bridge between human-readable specifications and machine-verifiable correctness guarantees.*

### Property Reflection

经过分析，我识别出以下可测试的属性，并进行了冗余性检查：

- 缩进计算属性可以合并为一个综合属性
- 菜单显示和交互属性可以分组处理
- 事件数据属性可以统一验证
- UI状态属性需要分别处理不同的交互场景

### Properties

**Property 1: 树形缩进计算**
*For any* 树形项目和其层级深度，项目的左侧缩进应该等于 depth * 16px
**Validates: Requirements 1.1, 1.2**

**Property 2: 文件夹展开状态图标**
*For any* 文件夹项目，当展开状态为true时应显示向下箭头图标，当展开状态为false时应显示向右箭头图标
**Validates: Requirements 1.3**

**Property 3: 菜单按钮悬停显示**
*For any* 树形项目，当鼠标悬停时菜单按钮应该可见，当鼠标离开时菜单按钮应该隐藏
**Validates: Requirements 2.1, 2.3**

**Property 4: 菜单按钮点击响应**
*For any* 菜单按钮，点击时应该显示对应的下拉菜单
**Validates: Requirements 2.2**

**Property 5: 项目类型菜单样式**
*For any* 项目类型（文件夹、服务、模式），其菜单按钮应该具有对应类型的特定样式属性
**Validates: Requirements 2.4**

**Property 6: 删除事件数据完整性**
*For any* 删除操作，发出的删除事件应该包含正确的项目ID和项目类型信息
**Validates: Requirements 4.3, 5.2**

**Property 7: 创建事件数据完整性**
*For any* 创建操作，发出的创建事件应该包含项目类型和父文件夹ID信息
**Validates: Requirements 5.1**

**Property 8: 导入事件数据完整性**
*For any* 导入操作，发出的导入事件应该包含目标文件夹信息
**Validates: Requirements 5.3**

**Property 9: 菜单事件上下文完整性**
*For any* 菜单操作事件，都应该包含必要的上下文信息（项目ID、类型、父级信息等）
**Validates: Requirements 5.4**

**Property 10: 视觉反馈状态一致性**
*For any* 项目的交互状态（悬停、活跃、点击），应该显示对应的视觉反馈样式
**Validates: Requirements 6.1, 6.2, 6.3**

**Property 11: 响应式宽度调整**
*For any* 侧边栏宽度变化，项目名称的显示宽度应该相应调整，保持菜单按钮可见
**Validates: Requirements 7.1, 7.3**

**Property 12: 长文本截断显示**
*For any* 超过显示宽度的项目名称，应该使用省略号进行截断显示
**Validates: Requirements 7.2**

## Error Handling

### 菜单操作错误处理

1. **无效项目ID**: 当菜单操作涉及不存在的项目ID时，应该忽略操作并记录警告
2. **权限检查**: 在执行删除操作前，应该检查项目是否可以被删除（例如，非空文件夹）
3. **状态不一致**: 当UI状态与数据状态不一致时，应该以数据状态为准并更新UI

### UI渲染错误处理

1. **缺失数据**: 当项目数据不完整时，应该显示默认值或占位符
2. **深度溢出**: 当树形结构层级过深时，应该限制最大缩进并显示警告
3. **菜单渲染失败**: 当菜单组件渲染失败时，应该降级到基本的右键菜单

## Testing Strategy

### 单元测试方法

**组件测试**:
- 测试TreeItem组件的渲染逻辑
- 测试ContextMenuButton的显示/隐藏逻辑
- 测试菜单选项的生成逻辑

**交互测试**:
- 测试鼠标悬停和点击事件的处理
- 测试菜单选择后的事件发送
- 测试键盘导航支持

**边界条件测试**:
- 测试空工作区的渲染
- 测试最大深度的树形结构
- 测试极长项目名称的处理

### 属性测试方法

**使用fast-check库进行属性测试**:
- 最小100次迭代验证每个属性
- 生成随机的树形结构数据进行测试
- 生成随机的用户交互序列进行测试

**测试数据生成器**:
```rust
// 生成随机的工作区数据
fn generate_workspace() -> Workspace
// 生成随机的树形结构
fn generate_tree_structure(max_depth: usize) -> Vec<TreeItem>
// 生成随机的用户交互序列
fn generate_user_interactions() -> Vec<UserInteraction>
```

**属性测试标签格式**:
每个属性测试使用注释标签：**Feature: workspace-sidebar-refactor, Property {number}: {property_text}**

### 集成测试

**完整工作流测试**:
- 测试从创建文件夹到添加服务的完整流程
- 测试导入OpenAPI后的树形结构更新
- 测试删除操作后的UI状态更新

**性能测试**:
- 测试大量项目时的渲染性能
- 测试频繁交互时的响应性能
- 测试内存使用情况
