# Requirements Document

## Introduction

本功能实现环境管理中的 base_url 与 HTTP 服务请求的集成。用户可以在环境中配置 base_url，并在发送 HTTP 请求时选择是否使用该 base_url 作为请求 URL 的前缀。这样可以方便地在不同环境（如开发、测试、生产）之间切换，同时保持灵活性以支持完整 URL 的直接输入。

## Glossary

- **Environment**: 环境配置，包含 base_url 和环境变量
- **Base_URL**: 环境中配置的基础 URL，如 `https://api.example.com`
- **HTTP_Service**: HTTP 请求服务组件，负责发送 HTTP 请求
- **Request_URL**: 用户在请求面板中输入的 URL 或路径
- **Final_URL**: 实际发送请求时使用的完整 URL
- **URL_Mode**: URL 模式，决定是否使用 base_url

## Requirements

### Requirement 1: 环境 Base URL 传递

**User Story:** As a developer, I want the active environment's base_url to be available in the HTTP service, so that I can use it when sending requests.

#### Acceptance Criteria

1. WHEN an environment is selected in the workspace, THE HTTP_Service SHALL have access to the active environment's base_url
2. WHEN the active environment changes, THE HTTP_Service SHALL receive the updated base_url
3. WHEN no environment is selected, THE HTTP_Service SHALL treat base_url as empty string

### Requirement 2: URL 模式选择

**User Story:** As a developer, I want to choose whether to prepend the base_url to my request path, so that I can flexibly handle both relative paths and absolute URLs.

#### Acceptance Criteria

1. THE HTTP_Service SHALL provide a toggle or selector to enable/disable base_url prepending
2. WHEN base_url mode is enabled, THE HTTP_Service SHALL display a visual indicator showing the mode is active
3. WHEN base_url mode is disabled, THE HTTP_Service SHALL send requests using only the user-entered URL
4. THE HTTP_Service SHALL persist the URL mode preference per service

### Requirement 3: URL 组合逻辑

**User Story:** As a developer, I want the system to correctly combine base_url and request path, so that I get valid URLs for my requests.

#### Acceptance Criteria

1. WHEN base_url mode is enabled AND user enters a relative path (e.g., `/users`), THE HTTP_Service SHALL combine base_url and path to form the final URL
2. WHEN base_url mode is enabled AND user enters an absolute URL (starting with `http://` or `https://`), THE HTTP_Service SHALL use the user-entered URL directly, ignoring base_url
3. WHEN combining URLs, THE HTTP_Service SHALL handle trailing/leading slashes correctly to avoid double slashes
4. WHEN base_url is empty AND base_url mode is enabled, THE HTTP_Service SHALL use the user-entered URL directly

### Requirement 4: UI 显示

**User Story:** As a developer, I want to see the final URL that will be used for the request, so that I can verify the request configuration before sending.

#### Acceptance Criteria

1. WHEN base_url mode is enabled, THE HTTP_Service SHALL display the combined final URL in the request info
2. WHEN hovering over the URL input, THE HTTP_Service SHALL show a tooltip with the final URL
3. THE HTTP_Service SHALL visually distinguish between the base_url portion and the path portion in the display

### Requirement 5: 请求信息记录

**User Story:** As a developer, I want the request info to show the actual URL used, so that I can debug and verify my requests.

#### Acceptance Criteria

1. WHEN a request is sent, THE HTTP_Service SHALL record the final URL (with or without base_url based on mode) in the request info panel
2. THE HTTP_Service SHALL clearly indicate whether base_url was used in the request info
