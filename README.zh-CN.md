# LocalRouter

LocalRouter 是一个面向本地开发服务的控制平面。

它会启动本地 daemon，监管项目进程，分配稳定的本地域名路由，并把同一份运行状态同时提供给 dashboard 和 CLI。

现在 daemon 会直接托管 dashboard，所以最终用户不需要再单独启动前端源码。

当前范围：

- 支持 macOS 和 Linux
- 仅支持本地 daemon
- 本地 HTTP API 监听在 `127.0.0.1`
- 本地代理域名基于 `.localhost`

## Monorepo 结构

```text
.
├── apps/
│   ├── dashboard/         # React dashboard
│   ├── localrouter-cli/   # CLI 客户端
│   └── localrouterd/      # daemon 二进制
└── crates/
    └── localrouter-core/  # 共享 Rust 后端核心逻辑
```

`localrouterd` 是唯一事实源。

- dashboard 通过本地 API 读取和修改 daemon 状态
- CLI 是同一套 API 的薄客户端
- core package 负责 manifest 解析、持久化、进程监管、路由生成、健康检查、日志和图谱状态

## 环境要求

- Rust toolchain
- Node.js 18+
- npm
- 如果你希望自动识别分支 / workspace 名称，建议安装 `git`

## 快速开始

### 最短路径

如果你已经安装好了 `localrouter` 和 `localrouterd`，进入你的项目目录后直接执行：

```bash
localrouter dev
```

这条命令会自动完成：

- 如果 `localrouterd` 没启动，就先启动它
- 自动导入或重扫当前项目
- 启动当前项目实例
- 默认打开内置 dashboard，除非你传 `--no-open`

内置 dashboard 由 daemon 直接提供，地址是：

```text
http://127.0.0.1:9731/
```

### 不下载源码直接安装

直接安装最新 release 二进制：

```bash
curl -fsSL https://raw.githubusercontent.com/AruNi-01/local-router/main/scripts/install/install.sh | sh
```

安装完成后直接执行：

```bash
localrouter dev
```

安装脚本会自动下载当前平台对应的 release 包，并在安装前校验 SHA-256。

### 1. 构建整个 Rust workspace

这一步只在你从源码开发时需要。

```bash
cargo build
```

### 2. 导入一个项目

进入你想代理的项目目录后，直接执行：

```bash
./target/debug/localrouter project add
```

或者显式指定路径：

```bash
./target/debug/localrouter project add /绝对路径/你的项目
```

如果 daemon 还没有启动，CLI 会自动把它拉起来。

默认 daemon 端点：

- API 服务：`http://127.0.0.1:9731/v1`
- Proxy 服务：`http://127.0.0.1:9730`

如果你想显式管理 daemon，也可以手动执行：

```bash
./target/debug/localrouter daemon start
./target/debug/localrouter daemon status
./target/debug/localrouter daemon stop
```

### 3. 启动一个项目或服务

启动某个项目下的所有匹配实例：

```bash
./target/debug/localrouter up my-project
```

只启动某个服务：

```bash
./target/debug/localrouter up dashboard
```

查看运行状态：

```bash
./target/debug/localrouter ps
./target/debug/localrouter routes
./target/debug/localrouter logs dashboard
```

直接打开一个可路由服务：

```bash
./target/debug/localrouter open dashboard
```

停止或重启：

```bash
./target/debug/localrouter down my-project
./target/debug/localrouter restart dashboard
```

### 4. 启动 dashboard

对最终用户来说，这一步不是必须的，因为 daemon 已经内置并托管了打包后的 dashboard。

```bash
cd apps/dashboard
npm install
npm run dev
```

默认 dashboard 会连接到 `http://127.0.0.1:9731/v1`。

如果 daemon API 地址不是默认值，可以这样覆盖：

```bash
VITE_LOCALROUTER_API=http://127.0.0.1:9731/v1 npm run dev
```

如果项目根目录存在 `localrouter.yaml`，LocalRouter 会直接使用它。

如果不存在，LocalRouter 会根据常见项目文件自动生成一份 manifest，并先保存在 daemon 状态里，直到你显式保存一份正式配置。

## 路由是怎么工作的

当某个服务配置了 route，daemon 会为它分配：

- 进程内部监听端口
- 对外稳定访问地址
- 一个或多个本地域名，通过 proxy 转发到实例

默认域名格式：

```text
<workspace>.<service>.<project>.localhost
```

如果项目当前只有一个激活 workspace，还会额外生成一个短域名别名：

```text
<service>.<project>.localhost
```

daemon 会返回最终可访问 URL；如果代理端口不是 80，会自动把端口带上。例如：

```text
http://main.dashboard.local-router.localhost:9730
http://dashboard.local-router.localhost:9730
```

dashboard 和 CLI 只消费 daemon 返回的 URL，不会在客户端自行拼接。

## 项目配置文件

项目配置文件位于项目根目录下的 `localrouter.yaml`。

最小示例：

```yaml
project: local-router
workspace:
  strategy: git-worktree
services:
  dashboard:
    command: npm run dev -- --host ${HOST} --port ${PORT}
    cwd: apps/dashboard
    protocol: http
    adapter: vite
    route: dashboard
    healthcheck: http://127.0.0.1:${PORT}
```

支持的顶层字段：

- `project`
- `workspace.strategy`
- `proxy.disabled`
- `services`

支持的服务字段：

- `command`
- `cwd`
- `protocol`
- `adapter`
- `route`
- `healthcheck`
- `env`
- `depends_on`
- `disabled`
- `language`

`command` 中可用的运行时变量：

- `${PORT}`：daemon 分配的内部端口
- `${HOST}`：当前固定为 `127.0.0.1`
- `${PUBLIC_URL}`：daemon 计算后的对外访问地址

行为说明：

- `route: none` 表示该服务不暴露公共代理路由
- HTTP 服务如果未配置 `healthcheck`，默认使用 `http://127.0.0.1:${PORT}`
- `disabled: true` 会保留服务定义，但把它标记为禁用
- `adapter` 默认会根据服务名和命令推断

## 自动检测

如果项目里没有 `localrouter.yaml`，LocalRouter 会尝试自动生成：

- 项目目录内 `package.json` 的 `dev` script
- 根目录 `Cargo.toml`
- 最后兜底为一个通用 Python HTTP 服务

当前内建适配器推断包括：

- `vite`
- `nextjs`
- `uvicorn`
- `cargo-bin`
- `generic`
- `worker`

自动检测只适合初始化接入。要获得稳定行为，建议尽快落一份真实的 `localrouter.yaml`。

## 全局 daemon 配置

daemon 的全局配置可以通过 dashboard 的 Settings 页面或 `/v1/config` API 修改。

重要字段：

- `apiPort`：默认 `9731`
- `proxyPort`：默认 `9730`
- `dnsSuffix`：默认 `.localhost`
- `logLevel`：默认 `info`
- `healthcheckInterval`：默认 `10`
- `autoDetect`：默认 `true`
- `hotReload`：默认 `false`

如果你修改了端口或 DNS 后缀，建议重启 daemon，并刷新 dashboard。

## CLI 命令参考

普通可读输出：

```bash
./target/debug/localrouter daemon start|stop|status
./target/debug/localrouter project add <path>
./target/debug/localrouter project list
./target/debug/localrouter project remove <id|name|path>
./target/debug/localrouter ps
./target/debug/localrouter up [target]
./target/debug/localrouter down [target]
./target/debug/localrouter restart <target>
./target/debug/localrouter logs <target>
./target/debug/localrouter routes
./target/debug/localrouter open <target>
./target/debug/localrouter doctor
./target/debug/localrouter graph
./target/debug/localrouter dev [path] [--no-open]
```

JSON 输出：

```bash
./target/debug/localrouter --json ps
./target/debug/localrouter --json routes
./target/debug/localrouter --json graph
```

CLI 对 target 的匹配比较宽松，支持：

- project id 或 name
- service id 或 name
- workspace id 或 name
- instance id
- instance URL 子串

## Dashboard 使用流

dashboard 展示的就是 daemon 当前状态：

- `Overview`：实例和路由概览
- `Projects`：导入、重扫、移除项目
- `Routes`：查看、筛选、复制、打开路由
- `Logs`：查看 daemon 聚合后的服务日志
- `Graph`：查看当前拓扑快照
- `Settings`：修改全局 daemon 配置和项目 manifest

## 持久化

daemon 状态保存在本地 SQLite 文件中。

典型位置：

- macOS：`~/Library/Application Support/localrouter/state.sqlite3`
- Linux：`~/.local/share/localrouter/state.sqlite3`

daemon 的 PID 文件也会保存在同一个本地数据目录里。

当前会持久化：

- projects
- workspaces
- service definitions
- instance 摘要
- routes
- manifest 快照
- daemon config

日志目前只保存在内存里，不做长期归档。

## 常见排障

### daemon 连不上

先检查：

```bash
./target/debug/localrouter daemon status
curl http://127.0.0.1:9731/v1/health
```

### 路由存在，但浏览器打不开

检查：

```bash
./target/debug/localrouter ps
./target/debug/localrouter routes
./target/debug/localrouter logs <service>
```

常见原因：

- 进程启动后立即退出
- healthcheck 失败
- route 处于 `conflict`
- 服务命令没有正确使用 `${PORT}`

### CLI 连到了错误的 daemon

可以显式覆盖 API 地址：

```bash
LOCALROUTER_API=http://127.0.0.1:9731/v1 ./target/debug/localrouter ps
```

### 项目导入结果不对

修好 `localrouter.yaml` 后重新导入：

```bash
./target/debug/localrouter project list
./target/debug/localrouter project remove /绝对路径/你的项目
./target/debug/localrouter project add /绝对路径/你的项目
```

## 开发说明

- 这是一个 monorepo，不要把 daemon / core 逻辑塞进 `apps/dashboard`
- `apps/localrouterd` 是 daemon 二进制
- `apps/localrouter-cli` 是 API 客户端
- `crates/localrouter-core` 是共享后端核心实现
- dashboard 应该消费 daemon 数据，而不是自己发明进程或路由状态
