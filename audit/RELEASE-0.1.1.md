# WhistleBox 0.1.1 验证与发布说明

日期：2026-09-07。本文补充此前的修复报告，区分实际执行结果与未覆盖范围。

## 验证环境

- Windows x64、Rust 1.98.1、Tauri 2.10.3、Edge / WebView2。
- 内置 Node.js 22.23.2，Whistle 2.10.9，qs 6.16.0；均使用锁文件，Node 下载校验 SHA-256。
- 本地测试在隔离目录运行；真实系统代理、信任证书及安装卸载只在 GitHub 托管的临时 Windows 环境测试。

## 已通过的本地验证

| 范围 | 结果 |
| --- | --- |
| React 回归 | 8 项通过；覆盖界面稳定、卸载前保存、错误传播、外部凭据与向导重试 |
| Rust 原生测试 | 13 项通过；覆盖配置合并、端口冲突、代理归属、认证边界、流式传输及 WebSocket |
| 静态检查 | TypeScript / Vite 构建、Rust 格式和严格 Clippy 通过 |
| Whistle 契约 | 启动配置隔离、受保护页面、规则导入导出、独立 CA 通过 |
| 进程共存 | 两实例独立运行、冲突保留外部进程、退出与父进程异常清理、上游代理转发通过 |
| 真实 Edge | 跨站嵌入、认证 Cookie、资源加载及脚本检查通过 |
| 真实 Tauri WebView2 | 六页面、持续嵌入、端口热更新与回滚、配置和规则往返、PAC 与 HTTPS 开关通过 |
| 外部实例 | 错误凭据拒绝、更新凭据后访问恢复、停止外部模式保留原实例 PID 通过 |
| 安装包构建 | 中文 NSIS 安装包生成成功 |

## Windows 系统功能

[GitHub 临时 Windows 环境的系统验证](https://github.com/fu9zhou/whistle-box/actions/runs/34087158796)已通过：全局代理、PAC 规则代理、恢复原设置、保留其他软件的后续修改、当前实例根证书安装与移除、重复安装保持归属，以及开机启动开关。

验证中解决了两类自动化环境差异：管理员 WebView2 宿主忽略调试环境变量，改用测试专属的机器级策略并在结束后恢复；根证书操作需要 Windows 确认，测试只处理属于应用子进程且包含完整测试证书指纹的窗口。测试辅助脚本通过 UTF-16 编码运行，以兼容 Windows PowerShell 5.1。正常使用保留 Windows 证书确认机制。

## 正式发布条件

[Windows 构建与发布工作流](https://github.com/fu9zhou/whistle-box/actions/workflows/windows.yml)对实际提交重新构建并执行以下检查；快速缓存诊断不能替代这些条件：

1. 版本一致性、前端回归与构建、Rust 格式、原生测试和 Clippy。
2. 真实 Whistle 契约、进程共存、Edge、Tauri WebView2 与 Windows 系统功能。
3. 下载已发布的 0.1.0 安装包，验证升级到当前版本及同版本覆盖安装。
4. 在独立目录验证全新安装，核对程序版本、Node 文件哈希和内置 Whistle 版本。
5. 分别验证升级与全新安装后的实际应用界面、配置操作和卸载。
6. npm 漏洞查询与 RustSec 公告检查。

全部通过后上传 Windows x64 安装包和 `SHA256SUMS.txt`；对应版本标签的所有发布条件通过后创建 GitHub Release。失败不会发布。发行文件的最终哈希以该 Release 附带的校验文件为准。

## 依赖检查

GitHub 的 npm 检查在升级后未报告漏洞。RustSec 中阻断发布的 9 条漏洞公告已通过移除未使用的 sysproxy 和更新兼容依赖修复；另将 rand 0.8 更新至修复条件性内存安全公告的 0.8.6。

上游仍有维护状态提示：GTK3 系列主要用于非 Windows 目标，另有 fxhash、proc-macro-error、UNIC 系列。glib 的条件性内存安全提示涉及非 Windows GTK 依赖。rand 0.7 来自 phf_codegen 等构建依赖，当前 Windows 依赖图未启用其 log 功能，不满足 RUSTSEC-2026-0097 所列触发条件。这些提示未隐藏，后续升级 Tauri 时需继续跟踪。

## 范围限制

- 本次未覆盖第三方 Whistle 插件、全部外部代理服务及全部 Windows 版本，相关问题需在对应环境复现验证。
- macOS / Linux 未进行发布验收，本版本发布 Windows x64 安装包。
- 项目介绍图的后续 logo 修改已按要求撤销，图片一致性问题暂缓处理。
- 更早的修复记录反映当时依赖和验收状态，当前版本应以本报告及对应 GitHub 工作流为准。
