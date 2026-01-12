# Design Document: OpenAPI 导入功能重构

## Overview

重构现有的 OpenAPI 导入功能，简化事件处理机制，支持从文件夹右键菜单触发导入，并支持 URL 导入。新设计参考 `import_dialog.rs` 的模式，采用更简洁的事件驱动架构。

## Architecture

### 现有架构问题

当前实现存在以下问题：
1. 事件类型过多：`ImportOpenApi`, `PreviewImport`, `RequestImportPreview`, `ResolveImportConflicts`
2. 事件处理分散在多个组件中
3. 导入触发入口不明确

### 新架构设计

```
┌─────────────────┐     ┌──────────────────┐     ┌───────────────────┐
│  Popup Menu     │────▶│  Import Dialog   │────▶│ WorkspaceRegistry │
│  (触发导入)      │     │  (配置和确认)     │     │  (执行导入)        │
└─────────────────┘     └──────────────────┘     └───────────────────┘
                              │
                              ▼
                        只发出两种事件:
                        - Import(config)
                        - Cancel
```

### 简化的事件流

1. 用户在 Popup Menu 中点击"导入 OpenAPI"
2. 打开 ImportDialog，预设目标文件夹
3. 用户输入内容（JSON/YAML 或 URL）
4. 点击"导入"发出 Import 事件
5. WorkspaceSidebar 处理事件，调用 WorkspaceRegistry 执行导入

## Components and Interfaces

### 1. 简化的 ImportDialog 组件

```rust
pub struct SimpleImportDialog {
    // 输入
    content_input: Entity<InputState>,
    
    // 目标文件夹（预设，只读显示）
    target_folder_id: Option<Uuid>,
    target_folder_name: Option<SharedString>,
    
    // 选项
    create_subfolders: bool,
    
    // 状态
    is_loading: bool,
    error_message: Option<String>,
    service_count: Option<usize>,
}

#[derive(Debug, Clone)]
pub enum SimpleImportDialogEvent {
    Import(SimpleImportConfig),
    Cancel,
}

#[derive(Debug, Clone)]
pub struct SimpleImportConfig {
    pub content: String,
    pub target_folder: Option<Uuid>,
    pub create_subfolders: bool,
}
```

### 2. 扩展的 Popup Menu

在 `popup_menu.rs` 中为服务文件夹和服务根节点添加"导入 OpenAPI"选项：

```rust
// 在 ServiceRoot 和 ServiceFolder 分支中添加
let import_openapi = Some(WorkspaceSidebarEvent::ShowImportDialog(folder_id));
```

### 3. WorkspaceSidebar 事件扩展

```rust
pub enum WorkspaceSidebarEvent {
    // ... 现有事件 ...
    ShowImportDialog(Option<Uuid>),  // 显示导入对话框，参数为目标文件夹 ID
    ExecuteImport(SimpleImportConfig), // 执行导入
}
```

### 4. URL 检测和下载

```rust
/// 检测输入是否为 URL
pub fn is_url(input: &str) -> bool {
    let trimmed = input.trim();
    trimmed.starts_with("http://") || trimmed.starts_with("https://")
}

/// 从 URL 下载 OpenAPI 内容
pub async fn fetch_openapi_from_url(url: &str) -> Result<String> {
    // 使用 reqwest 或类似库下载内容
}
```

## Data Models

### SimpleImportConfig

```rust
#[derive(Debug, Clone)]
pub struct SimpleImportConfig {
    /// OpenAPI 内容（JSON/YAML）或已下载的内容
    pub content: String,
    
    /// 目标文件夹 ID（None 表示使用默认的 tag 行为）
    pub target_folder: Option<Uuid>,
    
    /// 是否在目标文件夹下创建子文件夹
    pub create_subfolders: bool,
}
```

### ImportResult（复用现有）

```rust
pub struct ImportResult {
    pub folders: Vec<Folder>,
    pub services: Vec<Service>,
    pub schemas: Vec<Schema>,
}
```

## Correctness Properties

*A property is a characteristic or behavior that should hold true across all valid executions of a system-essentially, a formal statement about what the system should do. Properties serve as the bridge between human-readable specifications and machine-verifiable correctness guarantees.*

### Property 1: URL 检测正确性
*For any* 以 "http://" 或 "https://" 开头的字符串，`is_url()` 函数应该返回 true；对于其他字符串应该返回 false
**Validates: Requirements 6.1**

### Property 2: 目标文件夹导入正确性
*For any* 有效的 OpenAPI 规范和指定的目标文件夹，当禁用"创建子文件夹"选项时，所有导入的服务的 `folder_id` 应该等于目标文件夹 ID
**Validates: Requirements 4.1, 4.3**

### Property 3: 子文件夹创建正确性
*For any* 有效的 OpenAPI 规范和指定的目标文件夹，当启用"创建子文件夹"选项时，创建的子文件夹的 `parent_id` 应该等于目标文件夹 ID
**Validates: Requirements 4.2**

### Property 4: 无效内容错误处理
*For any* 无效的 OpenAPI 内容（空字符串、无效 JSON/YAML），解析函数应该返回错误
**Validates: Requirements 5.1**

### Property 5: 不存在的文件夹错误处理
*For any* 不存在的文件夹 ID，导入函数应该返回"目标文件夹不存在"错误
**Validates: Requirements 5.2**

### Property 6: 服务数量预览正确性
*For any* 有效的 OpenAPI 规范，预览显示的服务数量应该等于规范中定义的操作数量
**Validates: Requirements 2.4**

## Error Handling

### 输入验证错误
- 空内容：显示"请输入 OpenAPI 内容或 URL"
- 无效 JSON/YAML：显示解析错误详情
- 内容过大：显示"内容过大，请使用较小的规范文件"

### 网络错误（URL 导入）
- 连接失败：显示"无法连接到指定 URL"
- 超时：显示"下载超时，请检查网络连接"
- 非 200 响应：显示"服务器返回错误: {status_code}"

### 导入错误
- 目标文件夹不存在：显示"目标文件夹不存在"
- 数据库错误：显示"保存失败: {error_detail}"

## Testing Strategy

### 单元测试
- 测试 `is_url()` 函数的各种输入
- 测试 OpenAPI 解析的有效和无效输入
- 测试导入配置的验证逻辑

### 属性测试
- 使用 Rust 的 `proptest` 库
- 每个属性测试运行最少 100 次迭代
- 生成随机的 OpenAPI 规范和文件夹结构

### 集成测试
- 测试完整的导入流程
- 测试 URL 下载和导入的端到端流程
- 测试错误场景的处理
