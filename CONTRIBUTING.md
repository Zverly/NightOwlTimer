# 贡献指南

感谢你改进 NightOwl Timer。提交变更前，请先搜索现有 Issue，避免重复工作。

## 开发流程

1. Fork 仓库并从 `main` 创建功能分支。
2. 使用 Node.js 20.19 或更高版本和 Rust stable MSVC 工具链。
3. 运行 `npm ci` 安装锁定依赖。
4. 保持变更范围清晰，并为 Rust 行为变更补充测试。
5. 提交 Pull Request 前运行完整验证。

```powershell
npm run format:check
npm run typecheck
npm run build
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
cargo test --manifest-path src-tauri/Cargo.toml
```

## 提交与 Pull Request

- 提交信息使用简短的英文 Conventional Commit，例如 `fix: cancel task before exit`。
- 一个 Pull Request 只解决一个主题。
- 说明用户可见变化、测试方式和相关 Issue。
- UI 变更请提供修改前后的截图。
- 不要提交安装包、构建目录、日志、用户数据或计划任务 XML。

涉及关机、强制关机或睡眠的测试必须使用安全的依赖注入或模拟路径，不得在 CI 中执行真实系统动作。
