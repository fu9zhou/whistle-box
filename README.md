# WhistleBox

> 基于 [Whistle](https://github.com/nicosql/whistle) 的桌面代理调试工具，为 Whistle 提供开箱即用的原生桌面体验。

WhistleBox 将强大的 Whistle Web 调试代理封装为一个现代化的桌面应用，内置 Node.js 运行环境和 Whistle 实例，无需手动安装配置，一键启动即可使用。

## 特性

- **开箱即用** — 内置 Node.js + Whistle，安装后立即可用，无需任何前置依赖
- **系统代理集成** — 支持全局代理、规则代理（PAC）、直连三种模式，一键切换
- **内置 Whistle 界面** — Whistle UI 直接嵌入应用窗口，无需打开浏览器
- **HTTPS 抓包** — 自动管理根证书的安装和卸载，简化 HTTPS 调试流程
- **配置管理** — 支持多配置 Profile 导入导出，方便在不同场景间切换
- **规则编辑** — 内置规则编辑器，支持 Whistle 规则语法
- **外部模式** — 也可以连接已有的外部 Whistle 实例进行管理
- **开机自启** — 支持 Windows 开机自动启动
- **系统托盘** — 最小化到系统托盘，不占用任务栏空间
- **亮色/暗色主题** — 跟随系统或手动切换，UI 全面适配

## 安装

### Windows 用户

直接下载安装包：

1. 前往 [GitHub Releases](https://github.com/fu9zhou/whistle-box/releases) 页面
2. 下载最新版本的 `WhistleBox_x.x.x_x64-setup.exe`
3. 运行安装包，按提示完成安装
4. 启动 WhistleBox，按引导完成初始设置

### macOS 用户

目前暂无 macOS 构建版本。如果你是 Mac 用户并且有兴趣，非常欢迎参与贡献 macOS 版本的构建和测试（详见下方"贡献"章节）。

## 使用说明

### 内置模式（推荐）

1. 首次启动会进入设置引导，选择"内置模式"
2. WhistleBox 会自动启动内嵌的 Whistle 实例
3. 在 Whistle 页面中可以查看和修改抓包规则
4. 通过左侧栏的"代理控制"切换系统代理模式

### 外部模式

如果你已经在运行一个 Whistle 实例：

1. 在设置引导中选择"外部模式"
2. 填写已有 Whistle 实例的地址和端口
3. 如果 Whistle 设置了用户名密码，一并填写
4. WhistleBox 会通过 Auth Proxy 安全连接到你的 Whistle 实例

### 代理模式

| 模式 | 说明 |
|------|------|
| 直连 | 不设置系统代理，手动配置浏览器代理 |
| 全局代理 | 所有系统流量经过 Whistle |
| 规则代理（PAC） | 仅匹配规则的域名经过 Whistle，其余直连 |

## 技术栈

- **桌面框架**: [Tauri v2](https://v2.tauri.app/) (Rust + WebView2)
- **前端**: React 18 + TypeScript + Tailwind CSS + Zustand
- **后端**: Rust (Tokio async runtime)
- **代理核心**: [Whistle](https://github.com/nicosql/whistle) (Node.js)
- **打包**: NSIS (Windows Installer)

## 开发

### 环境要求

- [Node.js](https://nodejs.org/) >= 18
- [Rust](https://rustup.rs/) >= 1.75
- [Tauri 2 CLI](https://v2.tauri.app/start/prerequisites/)
- Windows 10/11 + WebView2 Runtime

### 快速开始

```bash
# 克隆仓库
git clone https://github.com/fu9zhou/whistle-box.git
cd whistle-box

# 安装依赖并准备 sidecar
npm run setup

# 启动开发模式
npm run start
```

### 常用命令

| 命令 | 说明 |
|------|------|
| `npm run setup` | 安装依赖 + 准备 sidecar (Node.js + Whistle) |
| `npm run start` | 准备 sidecar + 启动 Tauri 开发模式 |
| `npm run dev` | 仅启动 Vite 前端开发服务器 |
| `npm run package` | 完整打包流程（sidecar → build → NSIS） |
| `npm run format` | 格式化代码 |
| `npm run reset-config` | 重置配置并清理进程 |

### 项目结构

```
whistle-box/
├── src/                    # 前端 React 源码
│   ├── components/         # UI 组件
│   ├── stores/             # Zustand 状态管理
│   ├── styles/             # 全局样式
│   └── types.ts            # TypeScript 类型定义
├── src-tauri/              # Tauri/Rust 后端
│   ├── src/
│   │   ├── auth/           # Auth Proxy（认证代理）
│   │   ├── config/         # 配置管理
│   │   ├── proxy/          # 系统代理控制 + PAC 服务
│   │   ├── whistle/        # Whistle 进程管理
│   │   ├── tray.rs         # 系统托盘
│   │   ├── autostart.rs    # 开机自启
│   │   ├── utils.rs        # 工具函数
│   │   └── lib.rs          # Tauri 命令注册
│   ├── resources/whistle/  # 内嵌 Whistle 配置
│   ├── nsis-hooks.nsi      # NSIS 安装/卸载钩子
│   └── nsis-lang/          # NSIS 多语言文件
├── scripts/                # 构建脚本
└── site/                   # 项目官网
```

## 贡献

WhistleBox 欢迎任何形式的贡献！

### 特别欢迎

- **macOS 版本的构建和测试** — 项目基于 Tauri，理论上支持 macOS，但目前没有 Mac 环境进行测试。如果你是 Mac 用户，非常欢迎：
  - Fork 仓库并在 macOS 上尝试构建
  - 修复 macOS 平台特有的兼容性问题
  - 提交 PR 贡献 macOS 安装包构建流程

- **Bug 反馈和功能建议** — 在 [Issues](https://github.com/fu9zhou/whistle-box/issues) 中提交

### 贡献流程

1. Fork 本仓库
2. 创建你的分支 (`git checkout -b feature/amazing-feature`)
3. 提交更改 (`git commit -m 'Add amazing feature'`)
4. 推送到分支 (`git push origin feature/amazing-feature`)
5. 创建 Pull Request

> **提示**: 本项目代码完全使用 AI 编写（Claude Opus 4.6），你也可以使用 AI 辅助工具来参与开发和调试。

## 关于

### AI 驱动的开发

WhistleBox 是一个 **100% 由 AI 编写的项目**。从架构设计、Rust 后端、React 前端到 NSIS 安装脚本，所有代码均由 [Claude Opus 4.6](https://www.anthropic.com/claude) 生成。这是一个探索 AI 编程能力边界的实验性项目，同时也是一个实用的开发工具。

### 致谢

- [Whistle](https://github.com/nicosql/whistle) — 强大的跨平台 Web 调试代理工具，WhistleBox 的核心引擎
- [Tauri](https://v2.tauri.app/) — 构建轻量级桌面应用的现代框架
- [Anthropic Claude](https://www.anthropic.com/claude) — 驱动本项目开发的 AI 模型

## 许可证

MIT License
