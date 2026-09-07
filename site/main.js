const demos = {
  overview: {
    title: "一切就绪，开始调试。",
    description: "你的网络调试工作台，所有状态一目了然。",
    badge: "● Whistle 运行中",
    stats: [
      ["实例状态", "运行中", "内置 Whistle"],
      ["当前代理", "规则代理", "按域名智能分流"],
      ["HTTPS 证书", "已信任", "当前实例指纹匹配"],
    ],
    heading: "实例信息",
    note: "独立环境，开箱即用",
    rows: [
      ["运行模式", "内置实例"],
      ["代理地址", "127.0.0.1:18899"],
      ["运行环境", "Node.js + Whistle"],
    ],
  },
  proxy: {
    title: "流量去哪里，由你决定。",
    description: "Windows 支持三种代理模式，灵活适配不同场景。",
    badge: "● PAC 规则模式",
    stats: [
      ["直连", "释放接管", "尝试恢复原代理设置"],
      ["规则代理", "按需分流", "匹配域名走代理"],
      ["全局代理", "统一调试", "遵循系统代理的请求"],
    ],
    heading: "当前代理配置",
    note: "Windows 功能",
    rows: [
      ["代理目标", "127.0.0.1:18899"],
      ["匹配域名", "通过 Whistle"],
      ["其他域名", "直接连接"],
    ],
  },
  rules: {
    title: "只关注你关心的请求。",
    description: "使用 PAC 域名规则，管理哪些请求需要经过代理。",
    badge: "● 开发环境配置",
    stats: [
      ["多套配置", "随时切换", "适配不同项目"],
      ["导入规则", "快速复用", "接续已有配置"],
      ["导出配置", "轻松迁移", "分享前请先脱敏"],
    ],
    heading: "域名规则示例",
    note: "匹配时走代理",
    rows: [
      ["01", "*.example.com"],
      ["02", "api.local.test"],
      ["其他请求", "DIRECT"],
    ],
  },
  cert: {
    title: "HTTPS 调试，也能很清晰。",
    description: "检查当前实例的证书，避免在不同实例之间混用。",
    badge: "● 当前证书已信任",
    stats: [
      ["证书校验", "指纹匹配", "对应当前 Whistle"],
      ["信任范围", "当前用户", "Windows 证书存储"],
      ["移除范围", "归属明确", "仅移除本应用管理证书"],
    ],
    heading: "证书管理流程",
    note: "Windows 自动管理",
    rows: [
      ["第一步", "检查当前实例证书"],
      ["第二步", "按需安装并确认信任"],
      ["切换实例后", "重新检查证书状态"],
    ],
  },
};
const tabs = [...document.querySelectorAll("[data-demo]")];
const panel = document.querySelector("#demo-panel");
function selectDemo(tab) {
  const demo = demos[tab.dataset.demo];
  tabs.forEach((item) => {
    item.setAttribute("aria-selected", String(item === tab));
    item.tabIndex = item === tab ? 0 : -1;
  });
  panel.setAttribute("aria-labelledby", tab.id);
  // All demo content is authored locally; no remote or user input is inserted.
  panel.innerHTML = `<div class="demo-title-row"><h3>${demo.title}</h3><span class="demo-live">${demo.badge}</span></div><p>${demo.description}</p><div class="demo-stats">${demo.stats.map(([label, value, note]) => `<div class="demo-stat"><span>${label}</span><strong>${value}</strong><small>${note}</small></div>`).join("")}</div><div class="demo-info"><div class="demo-info-title">${demo.heading}<span>${demo.note}</span></div>${demo.rows.map(([label, value]) => `<div class="info-row"><span>${label}</span><b>${value}</b></div>`).join("")}</div>`;
}
tabs.forEach((tab, index) => {
  tab.addEventListener("click", () => selectDemo(tab));
  tab.addEventListener("keydown", (event) => {
    let next;
    if (event.key === "ArrowDown" || event.key === "ArrowRight") next = (index + 1) % tabs.length;
    if (event.key === "ArrowUp" || event.key === "ArrowLeft")
      next = (index - 1 + tabs.length) % tabs.length;
    if (event.key === "Home") next = 0;
    if (event.key === "End") next = tabs.length - 1;
    if (next !== undefined) {
      event.preventDefault();
      selectDemo(tabs[next]);
      tabs[next].focus();
    }
  });
});
const mobile = window.matchMedia("(max-width: 760px)");
function updateOrientation() {
  document
    .querySelector('[role="tablist"]')
    .setAttribute("aria-orientation", mobile.matches ? "horizontal" : "vertical");
}
mobile.addEventListener("change", updateOrientation);
updateOrientation();
selectDemo(tabs[0]);
