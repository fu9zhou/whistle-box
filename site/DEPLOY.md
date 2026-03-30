# WhistleBox 介绍网站部署教程

将 `site/index.html` 部署到 `fu9zhou.github.io/whistle-box`。

---

## 方案 A：GitHub Actions 自动部署（推荐）

每次 push 到 `master` 分支，自动将 `site/` 目录部署到 GitHub Pages。

### 步骤

1. **在项目根目录创建 workflow 文件** `.github/workflows/deploy-site.yml`：

```yaml
name: Deploy Site to GitHub Pages

on:
  push:
    branches: [master]
    paths:
      - "site/**"
  workflow_dispatch:

permissions:
  contents: read
  pages: write
  id-token: write

concurrency:
  group: pages
  cancel-in-progress: false

jobs:
  deploy:
    environment:
      name: github-pages
      url: ${{ steps.deployment.outputs.page_url }}
    runs-on: ubuntu-latest
    steps:
      - name: Checkout
        uses: actions/checkout@v4

      - name: Setup Pages
        uses: actions/configure-pages@v5

      - name: Upload artifact
        uses: actions/upload-pages-artifact@v3
        with:
          path: site

      - name: Deploy to GitHub Pages
        id: deployment
        uses: actions/deploy-pages@v4
```

2. **在 GitHub 仓库设置中启用 Pages**：
   - 打开仓库 → `Settings` → `Pages`
   - `Source` 选择 **GitHub Actions**
   - 保存

3. **推送代码**：

```bash
git add .github/workflows/deploy-site.yml site/
git commit -m "add: project introduction website with GitHub Pages deployment"
git push origin master
```

4. **等待部署完成**：
   - 打开仓库 → `Actions` 标签页
   - 等待 `Deploy Site to GitHub Pages` 工作流完成（通常 1-2 分钟）
   - 访问 `https://fu9zhou.github.io/whistle-box/`

### 后续更新

修改 `site/` 下的文件并 push 到 `master`，会自动触发重新部署。也可以在 `Actions` 页面手动触发（`workflow_dispatch`）。

---

## 方案 B：手动部署（docs 目录）

不需要 GitHub Actions，直接用 `docs/` 目录作为 Pages 源。

### 步骤

1. **将 site 内容复制到 docs 目录**：

```bash
# 在项目根目录执行
mkdir -p docs
cp site/index.html docs/index.html
```

2. **提交并推送**：

```bash
git add docs/
git commit -m "add: project website in docs/"
git push origin master
```

3. **在 GitHub 仓库设置中启用 Pages**：
   - 打开仓库 → `Settings` → `Pages`
   - `Source` 选择 **Deploy from a branch**
   - `Branch` 选择 `master`，目录选择 `/docs`
   - 点击 `Save`

4. **等待部署**：
   - 通常 1-3 分钟后生效
   - 访问 `https://fu9zhou.github.io/whistle-box/`

### 后续更新

修改 `docs/index.html` 并 push 即可。注意：如果你同时维护 `site/` 目录，记得同步到 `docs/`。

---

## 两种方案对比

|                | 方案 A (GitHub Actions)  | 方案 B (docs 目录)      |
| -------------- | ------------------------ | ----------------------- |
| **设置复杂度** | 需要创建 workflow 文件   | 只需复制文件            |
| **自动化**     | push 自动部署            | push 自动部署           |
| **源目录**     | 可以是任意目录 (`site/`) | 必须是 `/docs` 或根目录 |
| **灵活性**     | 可以加构建步骤           | 纯静态，无构建          |
| **推荐场景**   | 长期维护的项目           | 快速上线、简单项目      |

---

## 常见问题

### 页面 404？

- 确认 `Settings → Pages` 已正确配置
- 确认文件名是 `index.html`（区分大小写）
- 等待 1-3 分钟，GitHub Pages 部署有延迟

### 样式/字体加载不出来？

- 本网站使用 Google Fonts CDN，国内网络可能较慢
- 所有样式和脚本已内联在 HTML 中，不存在相对路径问题

### 想用自定义域名？

1. 在 `site/` 目录下创建 `CNAME` 文件，内容为你的域名（如 `whistlebox.example.com`）
2. 在域名 DNS 添加 CNAME 记录指向 `fu9zhou.github.io`
3. 在 `Settings → Pages → Custom domain` 填入域名
