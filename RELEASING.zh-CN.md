# 发布 LocalRouter

这份文档描述了当前仓库里已经落地的 LocalRouter 发布流程。

它覆盖：

- 如何改版本号
- 如何打 tag 并推送
- 如何让 GitHub Actions 构建并发布 release 资产
- 如何检查发布后的 assets
- 如何验证无源码安装链路
- 如何自动更新 Homebrew tap

英文版：

- [RELEASING.md](./RELEASING.md)

## 发布输入

当前发布链路依赖这些文件：

- [apps/localrouter-cli/Cargo.toml](./apps/localrouter-cli/Cargo.toml)
- [apps/localrouterd/Cargo.toml](./apps/localrouterd/Cargo.toml)
- [scripts/release/build-release.sh](./scripts/release/build-release.sh)
- [scripts/release/local-release.sh](./scripts/release/local-release.sh)
- [scripts/install/install.sh](./scripts/install/install.sh)
- [scripts/release/generate-homebrew-formula.sh](./scripts/release/generate-homebrew-formula.sh)
- [scripts/release/generate-homebrew-tap-readme.sh](./scripts/release/generate-homebrew-tap-readme.sh)
- [scripts/release/update-homebrew-tap.sh](./scripts/release/update-homebrew-tap.sh)
- [.github/release.yml](./.github/release.yml)
- [.github/workflows/release.yml](./.github/workflows/release.yml)

当前安装脚本默认指向这个 GitHub 仓库：

```text
https://github.com/AruNi-01/local-router
```

## 发布前检查

在打 tag 之前，先确认：

- 版本号已经更新
- release note 或 changelog 材料已经准备好
- `README.md` 和 `README.zh-CN.md` 仍然符合当前安装路径
- dashboard 仍然能正常构建
- daemon 仍然能托管内置 dashboard
- `localrouter dev` 端到端可用

## 1. 更新版本号

至少要更新：

- [apps/localrouter-cli/Cargo.toml](./apps/localrouter-cli/Cargo.toml)

建议一起保持同步：

- [apps/localrouterd/Cargo.toml](./apps/localrouterd/Cargo.toml)
- [crates/localrouter-core/Cargo.toml](./crates/localrouter-core/Cargo.toml)

版本号使用普通 semver，例如 `0.1.0`。Git tag 必须带 `v` 前缀，例如 `v0.1.0`。

## 2. 运行发布前检查

在仓库根目录执行：

```bash
cargo fmt --all --check
cargo check
cargo test -p localrouter-core
cd apps/dashboard && npm run build && cd ../..
```

建议再跑一遍真实产品路径：

```bash
cargo run -p localrouter-cli -- daemon stop
cargo run -p localrouter-cli -- dev --no-open
curl -I http://127.0.0.1:9731/
curl -I http://127.0.0.1:9731/v1/health
cargo run -p localrouter-cli -- daemon stop
```

可选的本地 release 包 smoke test：

最快的本地发布测试路径：

```bash
./scripts/release/local-release.sh
```

这条命令会为当前平台构建 release 包，并把本地发布目录放到：

- `dist/local-release`

同时它会打印一条可直接执行的 `file://` 本地安装测试命令。

Apple Silicon macOS：

```bash
./scripts/release/build-release.sh aarch64-apple-darwin darwin-arm64
```

Linux x64：

```bash
./scripts/release/build-release.sh x86_64-unknown-linux-gnu linux-x64
```

期望产物：

- `dist/localrouter-v<version>-<platform>.tar.gz`
- `dist/localrouter-v<version>-<platform>.tar.gz.sha256`

## 3. 提交发布改动

在打 tag 之前，先把版本号和文档改动提交掉。

示例：

```bash
git status
git add .
git commit -m "chore: release v0.1.0"
```

## 4. 创建并推送 tag

创建带注释的 tag：

```bash
git tag -a v0.1.0 -m "v0.1.0"
git push origin main
git push origin v0.1.0
```

注意：

- workflow 只有在 tag 匹配 `v*` 时才会执行 publish
- `workflow_dispatch` 只会跑构建矩阵，不会自动发布 GitHub Release

## 5. 观察 GitHub Actions

推送 tag 之后，打开：

- GitHub 仓库的 `Actions`
- workflow 名称：`release`

预期 job：

- `build`
- `publish`

预期构建平台：

- `linux-x64`
- `linux-arm64`
- `darwin-x64`
- `darwin-arm64`

`build` job 会：

- 构建 dashboard
- 构建 `localrouter` 和 `localrouterd`
- 打包成带版本号的 tar 包
- 生成 `.sha256`
- 上传 artifact

`publish` job 会：

- 下载所有 artifact
- 整理到 `release/`
- 生成 `latest` 别名
- 生成 `localrouter.rb`
- 生成 tap 用的 `README.md`
- 发布 GitHub Release assets
- 让 GitHub 自动生成 release notes
- 如果配置了 tap 自动化，还会自动更新 Homebrew tap
- 在 workflow summary 里写入一份精简发布结果

## 6. 检查 Release Assets

打开对应 tag 的 GitHub Release，确认这些 assets 都存在。

版本化资产：

- `localrouter-v0.1.0-linux-x64.tar.gz`
- `localrouter-v0.1.0-linux-x64.tar.gz.sha256`
- `localrouter-v0.1.0-linux-arm64.tar.gz`
- `localrouter-v0.1.0-linux-arm64.tar.gz.sha256`
- `localrouter-v0.1.0-darwin-x64.tar.gz`
- `localrouter-v0.1.0-darwin-x64.tar.gz.sha256`
- `localrouter-v0.1.0-darwin-arm64.tar.gz`
- `localrouter-v0.1.0-darwin-arm64.tar.gz.sha256`

latest 别名资产：

- `localrouter-latest-linux-x64.tar.gz`
- `localrouter-latest-linux-x64.tar.gz.sha256`
- `localrouter-latest-linux-arm64.tar.gz`
- `localrouter-latest-linux-arm64.tar.gz.sha256`
- `localrouter-latest-darwin-x64.tar.gz`
- `localrouter-latest-darwin-x64.tar.gz.sha256`
- `localrouter-latest-darwin-arm64.tar.gz`
- `localrouter-latest-darwin-arm64.tar.gz.sha256`

Homebrew formula：

- `localrouter.rb`
- `homebrew-tap-README.md`

Workflow summary：

- `publish` job 的 summary 应该包含 tag、release URL、release ID、asset 列表，以及 tap 更新状态

如果有任何一个缺失，不要对外宣布发布完成，先修 workflow。

## 7. 验证无源码安装链路

用指定版本验证安装脚本：

```bash
LOCALROUTER_VERSION=v0.1.0 \
LOCALROUTER_INSTALL_DIR="$(mktemp -d)" \
bash scripts/install/install.sh
```

然后验证安装后的二进制：

```bash
"$LOCALROUTER_INSTALL_DIR/localrouter" daemon status
```

建议再在干净 shell 里做一遍真实安装：

```bash
curl -fsSL https://raw.githubusercontent.com/AruNi-01/local-router/main/scripts/install/install.sh | sh
localrouter dev
```

要确认：

- archive 能正常下载
- checksum 校验通过
- `localrouter` 和 `localrouterd` 都被安装
- `localrouter dev` 能启动 daemon
- `http://127.0.0.1:9731/` 能打开内置 dashboard

如果你不想依赖 GitHub，也可以直接验证本地 staged release：

```bash
./scripts/release/local-release.sh
INSTALL_DIR="$(mktemp -d)"
LOCALROUTER_BASE_URL="file://$PWD/dist/local-release" \
LOCALROUTER_INSTALL_DIR="$INSTALL_DIR" \
bash scripts/install/install.sh
"$INSTALL_DIR/localrouter" daemon status
```

## 8. 自动更新 Homebrew tap

release workflow 现在支持在 GitHub Release 发布完成后自动更新 Homebrew tap。

自动化入口：

- [scripts/release/generate-homebrew-formula.sh](./scripts/release/generate-homebrew-formula.sh)
- [scripts/release/generate-homebrew-tap-readme.sh](./scripts/release/generate-homebrew-tap-readme.sh)
- [scripts/release/update-homebrew-tap.sh](./scripts/release/update-homebrew-tap.sh)

必须配置的 GitHub 仓库项：

- repo variable：`HOMEBREW_TAP_REPO`
  示例：`AruNi-01/homebrew-tap`
- repo secret：`HOMEBREW_TAP_TOKEN`
  这个 token 必须有权限 push 到 tap 仓库

可选 GitHub variables：

- `HOMEBREW_TAP_BRANCH`
  默认：`main`
- `HOMEBREW_TAP_FORMULA_PATH`
  默认：`Formula/localrouter.rb`
- `HOMEBREW_TAP_FORMULA_NAME`
  默认行为：`localrouter`
- `HOMEBREW_TAP_README_PATH`
  默认：`README.md`
- `HOMEBREW_TAP_UPDATE_README`
  默认：`true`
- `HOMEBREW_TAP_GIT_NAME`
  默认：`localrouter-bot`
- `HOMEBREW_TAP_GIT_EMAIL`
  默认：`localrouter-bot@users.noreply.github.com`

workflow 的自动更新行为：

1. 生成 `localrouter.rb`
2. 生成 tap 的 `README.md`
3. 发布 GitHub Release assets
4. clone 你配置的 tap 仓库
5. 覆盖目标 formula 和可选 README 文件
6. 如果内容有变化，就自动 commit 并 push

如果 `HOMEBREW_TAP_REPO` 或 `HOMEBREW_TAP_TOKEN` 没配，tap 更新步骤会安全跳过，不会导致发布失败。

GitHub 自动 release note：

- 发布时会让 GitHub 自动生成 release notes
- 分类规则配置在 [.github/release.yml](./.github/release.yml)
- 如果你希望 release note 分组更干净，就要在 PR 和提交流程里保持 label 使用一致

手动兜底方案：

1. 从 GitHub Release 下载 `localrouter.rb`
2. clone tap 仓库
3. 替换 `Formula/localrouter.rb`
4. commit 并 push

验证方式：

```bash
brew update
brew install <your-org-or-user>/tap/localrouter
localrouter daemon status
```

## 9. 对外发布

确认 assets 和安装路径都没问题之后，再做这些动作：

- 完善 GitHub Release note
- 对外给出安装命令
- 如有需要，给出固定版本安装命令

最新版本安装：

```bash
curl -fsSL https://raw.githubusercontent.com/AruNi-01/local-router/main/scripts/install/install.sh | sh
```

固定版本安装：

```bash
curl -fsSL https://raw.githubusercontent.com/AruNi-01/local-router/main/scripts/install/install.sh -o /tmp/localrouter-install.sh
LOCALROUTER_VERSION=v0.1.0 bash /tmp/localrouter-install.sh
```

## 10. 如果这次发布有问题

不要在用户已经看到某个版本后，悄悄复用同一个版本号重新发包。

推荐修复方式：

1. 先定位问题
2. 在 `main` 上合入修复
3. 重新发一个新 tag，例如 `v0.1.1`

如果当前 release 完全不可用，必须彻底移除：

```bash
git push --delete origin v0.1.0
git tag -d v0.1.0
```

同时手动删除 GitHub Release。

只有在你非常确定这个版本不应该被任何人继续使用时，才这么做。

## 备注

- `scripts/install/install.sh` 依赖 `latest` 资产名保留 `.tar.gz` 后缀
- 安装脚本会先做 SHA-256 校验，再解压
- daemon 会在 `http://127.0.0.1:9731/` 托管内置 dashboard
- 最终用户的安装路径依赖 GitHub Release assets，而不是源码仓库
