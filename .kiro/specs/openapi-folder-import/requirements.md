# Requirements Document

## Introduction

增强现有的 OpenAPI 导入功能，允许用户选择将导入的服务和模式导入到特定的文件夹中，而不是仅根据 tags 自动创建文件夹。这将为用户提供更好的组织控制能力。

## Glossary

- **OpenAPI_Importer**: 负责解析和导入 OpenAPI 规范的系统组件
- **Folder_Selector**: 用于选择目标文件夹的用户界面组件
- **Import_Dialog**: 显示导入选项的对话框界面
- **Target_Folder**: 用户选择的用于存放导入内容的目标文件夹
- **Service_Item**: 从 OpenAPI 规范导入的 HTTP 服务项目
- **Schema_Item**: 从 OpenAPI 规范导入的数据模式项目

## Requirements

### Requirement 1: 文件夹选择界面

**User Story:** 作为用户，我希望在导入 OpenAPI 文件时能够选择目标文件夹，以便将导入的内容组织到我想要的位置。

#### Acceptance Criteria

1. WHEN 用户启动 OpenAPI 导入流程 THEN THE Import_Dialog SHALL 显示文件夹选择选项
2. WHEN 用户查看文件夹选择器 THEN THE Folder_Selector SHALL 显示当前工作空间中所有可用的服务类型文件夹
3. WHEN 用户选择一个目标文件夹 THEN THE Import_Dialog SHALL 记录用户的选择
4. WHEN 用户未选择任何文件夹 THEN THE OpenAPI_Importer SHALL 使用默认的基于 tags 的文件夹创建行为

### Requirement 2: 导入到指定文件夹

**User Story:** 作为用户，我希望导入的服务能够放置在我选择的文件夹中，以便保持我的项目结构整洁。

#### Acceptance Criteria

1. WHEN 用户选择了目标文件夹并确认导入 THEN THE OpenAPI_Importer SHALL 将所有服务导入到指定的文件夹中
2. WHEN 导入服务到指定文件夹 THEN THE OpenAPI_Importer SHALL 忽略 OpenAPI 规范中的 tags 用于文件夹创建
3. WHEN 指定的目标文件夹不存在 THEN THE OpenAPI_Importer SHALL 返回错误并停止导入
4. WHEN 导入完成 THEN THE OpenAPI_Importer SHALL 在目标文件夹中显示所有导入的服务

### Requirement 3: 子文件夹创建选项

**User Story:** 作为用户，我希望能够选择是否在目标文件夹下根据 tags 创建子文件夹，以便在保持组织结构的同时控制导入位置。

#### Acceptance Criteria

1. WHEN 用户查看导入选项 THEN THE Import_Dialog SHALL 提供"创建子文件夹"的选项
2. WHEN 用户启用"创建子文件夹"选项 THEN THE OpenAPI_Importer SHALL 在目标文件夹下根据 tags 创建子文件夹
3. WHEN 用户禁用"创建子文件夹"选项 THEN THE OpenAPI_Importer SHALL 将所有服务直接放置在目标文件夹中
4. WHEN 创建子文件夹时 THEN THE OpenAPI_Importer SHALL 将子文件夹的 parent_id 设置为目标文件夹的 id

### Requirement 4: 模式导入处理

**User Story:** 作为用户，我希望导入的数据模式也能够放置在合适的位置，以便与服务保持一致的组织结构。

#### Acceptance Criteria

1. WHEN 用户选择目标文件夹进行导入 THEN THE OpenAPI_Importer SHALL 检查是否存在对应的模式类型文件夹
2. WHEN 存在同名的模式类型文件夹 THEN THE OpenAPI_Importer SHALL 将模式导入到该文件夹中
3. WHEN 不存在对应的模式类型文件夹 THEN THE OpenAPI_Importer SHALL 在目标文件夹的同级创建模式文件夹
4. WHEN 创建模式文件夹时 THEN THE OpenAPI_Importer SHALL 使用与目标文件夹相同的 parent_id

### Requirement 5: 导入预览功能

**User Story:** 作为用户，我希望在确认导入前能够预览将要导入的内容和目标位置，以便确认导入操作的正确性。

#### Acceptance Criteria

1. WHEN 用户选择了 OpenAPI 文件和目标文件夹 THEN THE Import_Dialog SHALL 显示导入预览
2. WHEN 显示导入预览 THEN THE Import_Dialog SHALL 列出将要导入的服务数量和名称
3. WHEN 显示导入预览 THEN THE Import_Dialog SHALL 列出将要导入的模式数量和名称
4. WHEN 显示导入预览 THEN THE Import_Dialog SHALL 显示目标文件夹路径和结构
5. WHEN 用户查看预览后 THEN THE Import_Dialog SHALL 提供确认或取消导入的选项

### Requirement 6: 错误处理和验证

**User Story:** 作为用户，我希望在导入过程中遇到问题时能够收到清晰的错误信息，以便了解如何解决问题。

#### Acceptance Criteria

1. WHEN 用户选择的目标文件夹被删除或不存在 THEN THE OpenAPI_Importer SHALL 显示"目标文件夹不存在"错误消息
2. WHEN 用户没有选择任何文件夹且禁用了默认行为 THEN THE OpenAPI_Importer SHALL 显示"请选择目标文件夹"错误消息
3. WHEN 导入过程中发生数据库错误 THEN THE OpenAPI_Importer SHALL 回滚所有更改并显示错误详情
4. WHEN 导入的服务名称与目标文件夹中现有服务冲突 THEN THE OpenAPI_Importer SHALL 提供重命名或跳过的选项