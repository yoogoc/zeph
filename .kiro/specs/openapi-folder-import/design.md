# Design Document: OpenAPI 文件夹导入功能

## Overview

本设计扩展现有的 OpenAPI 导入功能，增加文件夹选择能力。用户可以选择将导入的服务和模式放置到特定的文件夹中，而不是仅依赖于 OpenAPI 规范中的 tags 自动创建文件夹。这将为用户提供更好的项目组织控制能力。

## Architecture

### 现有架构分析

当前的 OpenAPI 导入流程：
1. 用户在 WorkspaceSidebar 中点击"Import OpenAPI"
2. 显示简单的文本输入对话框
3. 用户输入 OpenAPI 内容或 URL
4. 调用 `workspace::import_openapi()` 进行导入
5. 根据 tags 自动创建文件夹并导入服务和模式

### 新架构设计

增强后的导入流程：
1. 用户在 WorkspaceSidebar 中点击"Import OpenAPI"
2. 显示增强的导入对话框，包含：
   - OpenAPI 内容输入区域
   - 文件夹选择器
   - 导入选项（是否创建子文件夹）
   - 导入预览区域
3. 用户配置导入选项并预览
4. 调用增强的导入函数进行导入
5. 根据用户选择的选项进行导入

## Components and Interfaces

### 1. ImportDialog 组件

新的导入对话框组件，替换现有的简单文本输入对话框。

```rust
pub struct ImportDialog {
    // OpenAPI 内容输入
    content_input: Entity<InputState>,
    
    // 文件夹选择
    selected_folder: Option<Uuid>,
    available_folders: Vec<Folder>,
    
    // 导入选项
    create_subfolders: bool,
    
    // 预览数据
    preview_data: Option<ImportPreview>,
    
    // 状态
    is_loading: bool,
    error_message: Option<String>,
}

pub enum ImportDialogEvent {
    Import(ImportConfig),
    Cancel,
}

pub struct ImportConfig {
    pub content: String,
    pub target_folder: Option<Uuid>,
    pub create_subfolders: bool,
}

pub struct ImportPreview {
    pub services: Vec<ServicePreview>,
    pub schemas: Vec<SchemaPreview>,
    pub folders_to_create: Vec<String>,
}
```

### 2. FolderSelector 组件

文件夹选择器组件，显示可用的文件夹树。

```rust
pub struct FolderSelector {
    folders: Vec<Folder>,
    selected_folder: Option<Uuid>,
    expanded_folders: HashSet<Uuid>,
}

pub enum FolderSelectorEvent {
    SelectFolder(Option<Uuid>),
    ToggleFolder(Uuid),
}
```

### 3. 增强的 OpenAPI 导入函数

扩展现有的导入逻辑以支持目标文件夹选择。

```rust
pub struct ImportOptions {
    pub target_folder: Option<Uuid>,
    pub create_subfolders: bool,
}

pub fn import_openapi_with_options(
    workspace_id: Uuid,
    spec: &OpenApiSpec,
    options: ImportOptions,
) -> Result<ImportResult> {
    // 实现增强的导入逻辑
}
```

### 4. WorkspaceRegistry 扩展

在工作空间注册表中添加新的导入方法。

```rust
impl WorkspaceRegistry {
    pub fn import_openapi_with_options(
        &mut self,
        content: &str,
        options: ImportOptions,
        cx: &mut Context<Self>,
    ) -> Result<()> {
        // 调用增强的导入函数
    }
}
```

## Data Models

### ImportPreview 数据结构

```rust
#[derive(Debug, Clone)]
pub struct ServicePreview {
    pub name: String,
    pub method: String,
    pub path: String,
    pub tag: Option<String>,
    pub target_folder: String,
}

#[derive(Debug, Clone)]
pub struct SchemaPreview {
    pub name: String,
    pub schema_type: String,
    pub target_folder: String,
}
```

### ImportOptions 扩展

```rust
#[derive(Debug, Clone)]
pub struct ImportOptions {
    pub target_folder: Option<Uuid>,
    pub create_subfolders: bool,
    pub schema_folder_strategy: SchemaFolderStrategy,
}

#[derive(Debug, Clone)]
pub enum SchemaFolderStrategy {
    SameAsServices,      // 与服务放在同一文件夹
    SeparateFolder,      // 创建单独的模式文件夹
    ExistingSchemaFolder(Uuid), // 使用现有的模式文件夹
}
```

## Correctness Properties

*A property is a characteristic or behavior that should hold true across all valid executions of a system-essentially, a formal statement about what the system should do. Properties serve as the bridge between human-readable specifications and machine-verifiable correctness guarantees.*

基于需求分析，以下是可测试的正确性属性：

### Property 1: 文件夹选择器显示正确的文件夹
*For any* 工作空间和文件夹集合，文件夹选择器应该只显示服务类型的文件夹
**Validates: Requirements 1.2**

### Property 2: 文件夹选择状态管理
*For any* 文件夹选择操作，对话框应该正确记录和维护用户的选择状态
**Validates: Requirements 1.3**

### Property 3: 默认导入行为保持一致
*For any* OpenAPI 规范，当未选择目标文件夹时，导入行为应该与原有的基于 tags 的逻辑完全一致
**Validates: Requirements 1.4**

### Property 4: 服务导入到指定文件夹
*For any* OpenAPI 规范和有效的目标文件夹，所有导入的服务都应该具有正确的 folder_id
**Validates: Requirements 2.1**

### Property 5: 忽略 tags 进行文件夹创建
*For any* OpenAPI 规范和指定的目标文件夹，当选择目标文件夹时，不应该根据 tags 创建新的文件夹
**Validates: Requirements 2.2**

### Property 6: 无效文件夹错误处理
*For any* 不存在的文件夹 ID，导入操作应该返回适当的错误并停止执行
**Validates: Requirements 2.3**

### Property 7: 子文件夹创建逻辑
*For any* OpenAPI 规范和目标文件夹，当启用"创建子文件夹"选项时，应该在目标文件夹下根据 tags 创建正确的子文件夹结构
**Validates: Requirements 3.2**

### Property 8: 直接导入到目标文件夹
*For any* OpenAPI 规范和目标文件夹，当禁用"创建子文件夹"选项时，所有服务应该直接放置在目标文件夹中
**Validates: Requirements 3.3**

### Property 9: 子文件夹层次结构
*For any* 创建的子文件夹，其 parent_id 应该等于目标文件夹的 id
**Validates: Requirements 3.4**

### Property 10: 模式文件夹检查
*For any* 目标文件夹，导入时应该正确检查是否存在对应的模式类型文件夹
**Validates: Requirements 4.1**

### Property 11: 模式导入到现有文件夹
*For any* 存在同名模式文件夹的情况，模式应该导入到该现有文件夹中
**Validates: Requirements 4.2**

### Property 12: 模式文件夹创建
*For any* 不存在对应模式文件夹的情况，应该在目标文件夹的同级创建新的模式文件夹
**Validates: Requirements 4.3**

### Property 13: 模式文件夹层次结构
*For any* 创建的模式文件夹，其 parent_id 应该与目标文件夹的 parent_id 相同
**Validates: Requirements 4.4**

### Property 14: 预览服务信息准确性
*For any* OpenAPI 规范，导入预览应该包含正确的服务数量、名称和相关信息
**Validates: Requirements 5.2**

### Property 15: 预览模式信息准确性
*For any* OpenAPI 规范，导入预览应该包含正确的模式数量、名称和相关信息
**Validates: Requirements 5.3**

### Property 16: 预览文件夹路径显示
*For any* 目标文件夹选择，预览应该显示正确的文件夹路径和结构信息
**Validates: Requirements 5.4**

### Property 17: 错误消息准确性
*For any* 错误条件（如不存在的文件夹、缺少选择等），应该返回相应的准确错误消息
**Validates: Requirements 6.1, 6.2**

### Property 18: 名称冲突处理
*For any* 导入操作中的服务名称冲突，应该提供适当的处理选项（重命名或跳过）
**Validates: Requirements 6.4**

## Error Handling

### 用户输入验证
- 验证 OpenAPI 内容格式的有效性
- 验证目标文件夹的存在性和有效性
- 验证文件夹类型匹配（服务文件夹用于服务导入）

### 导入过程错误处理
- 解析 OpenAPI 规范失败时的错误提示
- 数据库操作失败时的事务回滚
- 网络请求失败时的重试机制（对于 URL 导入）

### 冲突解决策略
- 服务名称冲突：提供重命名或跳过选项
- 文件夹名称冲突：自动添加数字后缀
- 模式名称冲突：合并或替换选项

### 错误消息设计
- 提供清晰、可操作的错误消息
- 包含错误原因和建议的解决方案
- 支持多语言错误消息

## Testing Strategy

### 单元测试
- 测试 ImportDialog 组件的状态管理
- 测试 FolderSelector 组件的文件夹过滤逻辑
- 测试导入预览数据的生成准确性
- 测试错误处理和验证逻辑

### 属性测试
- 使用 Rust 的 `proptest` 库进行属性测试
- 每个属性测试运行最少 100 次迭代
- 生成随机的 OpenAPI 规范和文件夹结构进行测试
- 验证导入结果的正确性和一致性

### 集成测试
- 测试完整的导入流程
- 测试 UI 组件之间的交互
- 测试数据库操作的正确性
- 测试错误场景的端到端处理

### 测试数据生成
- 创建有效和无效的 OpenAPI 规范样本
- 生成各种文件夹结构场景
- 模拟不同的用户选择组合

### 性能测试
- 测试大型 OpenAPI 规范的导入性能
- 测试文件夹选择器在大量文件夹时的响应性能
- 测试预览生成的性能

<function_calls>
<invoke name="prework">
<parameter name="featureName">openapi-folder-import