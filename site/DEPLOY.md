# WhistleBox 官网

纯静态官网，无需安装依赖或构建。将本目录的全部内容作为 GitHub Pages 发布产物即可，网站地址为 `https://fu9zhou.github.io/whistle-box/`。样式、脚本、图片使用相对路径，支持 `/whistle-box/` 子目录。

## 本地预览

在项目根目录运行 `python -m http.server 4173 --directory site`，打开 `http://localhost:4173/`。

## 后续部署

GitHub Pages 设置中选择 GitHub Actions 作为发布来源。`.github/workflows/pages.yml` 会在 `main` 分支的 `site/**` 或该工作流变更时，自动上传 `site` 目录并发布；也可以在 Actions 的“官网发布”工作流中手动运行。无需构建，不影响桌面应用产物。

## 内容与维护

- `index.html`：中文产品介绍、功能、平台下载、上手流程、FAQ。
- `styles.css`：米白与品牌绿色设计、桌面及移动布局、焦点与减少动画支持。
- `main.js`：功能演示标签页，支持鼠标、触摸、方向键、Home 与 End。
- `assets`：复用项目 Logo，保持原图比例与颜色。
- 下载入口指向真实 GitHub 最新发行页面，不硬编码版本或猜测附件名。macOS/Linux 明确标识实验性，提供 Actions 开发构建备用入口。
- 功能预览为交互示意，使用静态演示数据，不是应用截图，不会操作系统代理或证书。

设计参考 ui-ux-pro-max 的产品下载页结构与简约风格建议；按项目 Logo 调整为品牌绿色，使用系统字体以减少外部资源与加载延迟。
