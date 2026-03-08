# LocalRouter 计划

日期：2026-03-08
状态：草案
产品形态：以 Daemon 为核心的本地开发平台

## 1. 产品概述

LocalRouter 是一个面向并行 worktree 和多服务项目的本地开发运行时。

它解决的问题：

- 为每个服务和每个 worktree 提供稳定的本地 URL
- 在项目、分支、工作空间之间自动隔离端口
- 支持跨语言服务启动与进程托管
- 为人类、CLI、IDE、AI Agent 提供统一的服务发现能力
- 通过可视化方式理解项目、工作空间、服务、路由和依赖关系

LocalRouter 不只是一个反向代理。它是一个由 daemon 管理的本地控制平面，包含：

- 后台 daemon
- CLI
- 本地反向代理
- 服务运行器与适配器系统
- Web UI 看板
- 项目与工作空间拓扑图模型

## 2. 产品目标

### 核心目标

- 让并行 worktree 开发变成稳定、低摩擦、无需手工协调的流程
- 消除手动分配端口的成本
- 为每个服务提供稳定、可命名的本地域名
- 支持异构技术栈：Node.js、Python、Go、Rust、Java，以及任意自定义命令
- 让用户一眼看到当前本地环境状态

### 次级目标

- 为 Agent 和编辑器集成提供机器可读的服务元数据
- 让日志、健康状态、路由和服务关系更容易排查和理解
- 支持团队约定，但不依赖项目自己的 task runner

### v1 非目标

- 远程部署
- 云端服务注册中心
- Kubernetes 级别的编排
- 在 core daemon 稳定前就优先做完整 IDE 插件生态

## 3. 核心概念

- Project：一个仓库或本地项目根目录
- Workspace：一个具体的工作上下文，通常对应 git worktree 或本地 checkout
- Service：一个可运行单元，例如 web、api、worker、docs、db proxy、cron
- Route：映射到服务实例的稳定本地 URL
- Instance：某个 workspace 中某个 service 的一个运行实例
- Graph：项目、工作空间、服务、路由之间的关系模型
- Adapter：知道如何启动或参数化某类服务的适配逻辑

## 4. 产品组成

### Daemon

Daemon 是全局唯一的事实来源，负责维护：

- 进程注册表
- 路由注册表
- 端口分配状态
- workspace 身份信息
- 健康状态
- 日志元数据
- 图谱状态

它需要暴露：

- 本地 HTTP API
- 本地 WebSocket 事件流
- 可选的本地 Unix socket，供 CLI 做可信通信

### CLI

CLI 是用户的操作入口。

核心命令：

- `localrouter up`
- `localrouter down`
- `localrouter ps`
- `localrouter logs <service>`
- `localrouter open <service>`
- `localrouter doctor`
- `localrouter graph`
- `localrouter project add`
- `localrouter workspace use`

CLI 必须独立于 npm scripts、just、make、cargo alias 等项目内部脚本系统。

### Proxy

Proxy 提供稳定的本地域名，例如：

- `web.myapp.localhost`
- `api.myapp.localhost`
- `docs.myapp.localhost`
- `feat-login.api.myapp.localhost`

职责包括：

- 注册和注销路由
- HTTP 与 WebSocket 转发
- 基于 Host 的路由分发
- 后续可扩展本地 HTTPS

### Web UI

Web UI 是主要的可视化界面。

核心视图：

- Projects 列表
- Project 详情
- Workspace 详情
- Service 实例详情
- Route 检查器
- 日志查看器
- Graph 视图

图谱视图要求：

- 展示 Project -> Workspace -> Service 的层级关系
- 展示服务之间的依赖
- 展示路由绑定关系
- 展示运行状态与健康状态
- 支持按 project、workspace、route、health、language 过滤

## 5. 配置模型

LocalRouter 应支持两层配置：

### A. 自动检测

用于初始化和最佳努力推断。

检测来源包括：

- `package.json`
- `Cargo.toml`
- `go.mod`
- `pyproject.toml`
- `pom.xml` / `build.gradle`
- Docker Compose 文件

自动检测只负责辅助，不应成为最终真相来源。

### B. 显式 manifest

最终应以 manifest 作为项目意图的权威来源。

建议文件：

- `localrouter.yaml`

示例结构：

```yaml
project: myapp

workspaces:
  strategy: git-worktree

services:
  web:
    command: next dev --port ${PORT}
    protocol: http
    route: web
    healthcheck: http://127.0.0.1:${PORT}

  api:
    command: cargo run --bin api -- --port ${PORT}
    protocol: http
    route: api
    healthcheck: http://127.0.0.1:${PORT}/healthz

  worker:
    command: python worker.py
    route: none
```

关键设计原则：

- 不要求项目必须依赖自己的 task runner
- 永远支持自定义命令
- 适配器存在的意义只是减少样板代码

## 6. Adapter 适配策略

之所以需要 Adapter，是因为不同语言和框架对端口、host、public URL 的接入方式并不统一。

Adapter 的职责：

- 注入端口和 host
- 为常见框架自动追加已知启动参数
- 生成 public URL 环境变量
- 提供默认健康检查
- 尽可能解析启动成功信号

初始适配目标：

- Next.js / Vite / 通用 Node
- Uvicorn / FastAPI / 通用 Python
- Rust 自定义命令
- Spring Boot / JVM 应用
- Go 自定义命令

兜底方案：

- 原始命令适配器，支持 `${PORT}`、`${HOST}`、`${PUBLIC_URL}` 插值

## 7. 命名与路由策略

默认路由格式：

- `<service>.<project>.localhost`

带 worktree 的路由格式：

- `<workspace>.<service>.<project>.localhost`

示例：

- `main.web.atmos.localhost`
- `feat-auth.api.atmos.localhost`
- `docs.localrouter.localhost`

命名规则必须满足：

- 可预测
- 可读
- 避免冲突
- 支持覆盖

## 8. 运行时架构

### 内部组件

- Registry 服务
- 端口分配器
- 进程监督器
- Proxy 管理器
- Workspace 解析器
- 健康检查引擎
- 事件总线
- 图谱构建器
- 持久化层

### 持久化内容

需要本地持久化：

- 已知项目
- 已知 workspaces
- 最近的路由分配
- 服务定义
- daemon 运行状态

不在 v1 持久化：

- 长期日志存档
- 完整终端录制

## 9. Web UI 信息架构

### 首页看板

- 所有活跃项目
- 活跃 workspaces
- 正在运行的服务
- 不健康服务
- 路由冲突

### Project 页面

- 项目元数据
- workspace 列表
- 服务目录
- 默认图谱视图

### Workspace 页面

- branch/worktree 身份信息
- 正在运行的服务
- 路由列表
- 日志与健康状态
- 当前 workspace 的图谱切片

### Service 页面

- 启动命令
- 适配器类型
- 分配端口
- 对外 URL
- 健康检查
- 依赖关系
- 日志
- 重启与停止控制

### Graph 页面

- 力导向图或分层图
- 节点类型：project、workspace、service、route
- 边类型：contains、depends_on、exposes、proxies_to

## 10. API 设计

Daemon API 应采用本地优先、事件驱动的设计。

最小 API 资源组：

- `/projects`
- `/workspaces`
- `/services`
- `/instances`
- `/routes`
- `/graph`
- `/logs`
- `/health`
- `/events`

事件流至少包括：

- service_started
- service_stopped
- service_failed
- health_changed
- route_registered
- route_removed
- workspace_detected

## 11. 安全模型

这是一个本地工具，但仍然需要明确边界。

- daemon 默认只监听 localhost
- CLI / UI 与 daemon 之间需要可信通信边界
- 默认不开放远程访问
- 避免危险的 shell 插值
- manifest 解析不能执行任意代码
- 路由名称必须做安全清洗

## 12. 实施阶段

### Phase 1：Daemon Core

- daemon 进程
- registry
- 端口分配器
- 进程监督器
- 原始命令执行
- 本地持久化

### Phase 2：Proxy 与路由

- 基于 Host 的本地代理
- 路由注册
- HTTP 与 WebSocket 转发
- 稳定本地域名生成

### Phase 3：Manifest 与 Adapter

- `localrouter.yaml`
- 项目自动检测
- adapter API
- 主流技术栈的一方适配器

### Phase 4：CLI 完整化

- 生命周期命令
- 日志查看
- open
- doctor
- graph 导出

### Phase 5：Web UI

- 看板
- project/workspace/service 详情页
- WebSocket 实时更新
- 图谱浏览器

### Phase 6：图谱智能化

- 服务依赖边
- route-to-service 边
- workspace 对比
- 冲突检测

### Phase 7：Agent 与编辑器集成

- 机器可读上下文接口
- 面向 agent 的 CLI 输出模式
- 编辑器深链

## 13. MVP 与最终形态的关系

虽然这个产品按最终 daemon-first 形态设计，但第一版可交付范围仍然应该收敛：

- daemon
- proxy
- 原始命令运行器
- manifest
- 基础 CLI
- 最小可用 dashboard

图谱智能、深度 adapter、编辑器集成，都应建立在稳定 daemon 基元之上。

## 14. 主要风险

- 框架适配器数量膨胀
- 用户对“零配置支持任意栈”有过高预期
- worktree 场景下路由命名稳定性难保证
- macOS / Linux 上进程生命周期边界复杂
- 图谱可能过于复杂，反而降低可读性

## 15. 成功标准

- 开发者可以同时运行同一项目的两个或以上 worktree，而无需手工选端口
- 开发者可以通过命名本地域名访问每个服务
- 混合语言技术栈可以通过 manifest 启动，而不依赖项目内部 task runner
- dashboard 能实时反映 project/workspace/service 状态
- Agent 可以从 daemon 中准确找到对应的本地服务 URL

## 16. 推荐的下一步文档

- daemon / process model 的 architecture decision record
- `localrouter.yaml` schema 草案
- CLI command spec
- daemon API spec
- Web UI dashboard 与 graph 线框计划
- adapter contract spec
