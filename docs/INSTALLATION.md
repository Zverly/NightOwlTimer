# 安装与发布说明

## 普通用户安装

1. 从 [最新 Release](https://github.com/Zverly/NightOwlTimer/releases/latest) 下载 NSIS
   (`*-setup.exe`) 或 MSI (`*.msi`) 安装包。
2. Windows 10/11 x64 用户可直接运行安装程序，按向导选择安装目录。
3. 首次运行后，可在“设置 → 关于 NightOwl”确认版本、运行平台和数据目录。

安装包暂未使用代码签名证书，SmartScreen 可能显示“未知发布者”。请确认文件来自本仓库
Release，并优先使用同一 Release 中的 `SHA256SUMS.txt` 校验完整性。

## 校验安装包

在 PowerShell 中进入下载目录，执行：

```powershell
Get-FileHash '.\NightOwl Timer_1.0.1_x64-setup.exe' -Algorithm SHA256
Get-FileHash '.\NightOwl Timer_1.0.1_x64_en-US.msi' -Algorithm SHA256
```

将输出的哈希值与 `SHA256SUMS.txt` 对比。文件名可能随版本变化，请以 Release 页面中的
实际文件名为准。

## 升级与数据

- 应用数据位于安装目录的 `data\nightowl-data.json`。
- 升级安装不会主动删除数据；卸载程序也不会自动删除该目录。
- 从旧版固定路径升级时，应用会在新位置为空时自动迁移兼容数据。
- 需要备份时，退出 NightOwl 后复制整个 `data` 文件夹即可。

## 从源码构建

```powershell
npm ci
npm run tauri build -- --bundles nsis,msi
```

产物位于 `src-tauri\target\release\bundle\`。推送 `v*` 标签后，GitHub Actions 会自动
构建 NSIS、MSI，生成校验文件并创建 Release。

## 常见问题

- **无法打开仓库**：确认系统已设置默认浏览器；也可复制仓库地址到浏览器手动访问。
- **本地数据入口打开位置不对**：请升级到 `1.0.1`，新版会打开应用目录下的 `data` 文件夹，
  而不是“文档”目录或 JSON 文件关联程序。
- **关闭窗口后任务仍在执行**：这是默认的“关闭时隐藏到托盘”行为，可在设置中关闭。
