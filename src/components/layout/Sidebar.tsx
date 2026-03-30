import { LayoutDashboard, Globe, ListFilter, Terminal, FolderCog, Settings } from "lucide-react";
import type { Page } from "../../types";
import { useAppStore } from "../../stores/appStore";

interface SidebarProps {
  currentPage: Page;
  onNavigate: (page: Page) => void;
}

const navItems: { id: Page; label: string; icon: typeof LayoutDashboard }[] = [
  { id: "dashboard", label: "仪表盘", icon: LayoutDashboard },
  { id: "whistle", label: "Whistle", icon: Terminal },
  { id: "proxy", label: "代理", icon: Globe },
  { id: "rules", label: "规则", icon: ListFilter },
  { id: "config", label: "配置", icon: FolderCog },
  { id: "settings", label: "设置", icon: Settings },
];

export default function Sidebar({ currentPage, onNavigate }: SidebarProps) {
  const whistleStatus = useAppStore((s) => s.whistleStatus);
  const proxyStatus = useAppStore((s) => s.proxyStatus);
  const config = useAppStore((s) => s.config);

  const whistleAlive = whistleStatus?.running && whistleStatus?.uptime_check;
  const isEmbedded = config?.whistle?.mode === "embedded";

  return (
    <aside className="w-[200px] shrink-0 flex flex-col themed-sidebar">
      <div className="px-3 pt-4 pb-2">
        <div className="glass-panel p-3 space-y-2">
          <div className="flex items-center gap-2">
            <span
              className={`w-2 h-2 rounded-full ${
                proxyStatus?.mode === "global"
                  ? "bg-emerald-400"
                  : proxyStatus?.mode === "rule"
                    ? "bg-blue-400"
                    : "bg-surface-500"
              }`}
            />
            <span className="text-xs text-surface-400">
              {proxyStatus?.mode === "global"
                ? "全局模式"
                : proxyStatus?.mode === "rule"
                  ? "规则模式"
                  : "直连模式"}
            </span>
          </div>
          <div className="flex items-center gap-2">
            <span
              className={`status-dot ${whistleAlive ? "status-dot--active animate-pulse-soft" : "status-dot--inactive"}`}
            />
            <span className="text-xs text-surface-400">
              {isEmbedded
                ? whistleAlive
                  ? "内置 Whistle 运行中"
                  : "内置 Whistle 未启动"
                : whistleAlive
                  ? "外部 Whistle 已连接"
                  : "外部 Whistle 未连接"}
            </span>
          </div>
        </div>
      </div>

      <nav className="flex-1 px-2 py-2 space-y-0.5">
        {navItems.map((item) => {
          const Icon = item.icon;
          const isActive = currentPage === item.id;
          return (
            <button
              key={item.id}
              onClick={() => onNavigate(item.id)}
              aria-current={isActive ? "page" : undefined}
              className={`w-full nav-item ${isActive ? "nav-item--active" : ""}`}
            >
              <Icon size={18} strokeWidth={isActive ? 2 : 1.5} />
              <span className="text-[13px]">{item.label}</span>
              {item.id === "whistle" && whistleAlive && (
                <span className="ml-auto status-dot status-dot--active" />
              )}
            </button>
          );
        })}
      </nav>

      <div className="px-3 pb-3">
        <div className="text-[10px] themed-text-muted text-center font-mono">
          Powered by Whistle & Tauri
        </div>
      </div>
    </aside>
  );
}
