import { useEffect, useState } from "react";
import { Globe, Shield, Unplug, AlertCircle } from "lucide-react";
import { useAppStore } from "../../stores/appStore";

type ProxyMode = "global" | "rule" | "direct";

const modes: {
  id: ProxyMode;
  label: string;
  sublabel: string;
  icon: typeof Globe;
  color: string;
  bg: string;
  ring: string;
}[] = [
  {
    id: "global",
    label: "全局模式",
    sublabel: "所有系统流量通过 Whistle 代理",
    icon: Globe,
    color: "text-emerald-400",
    bg: "bg-emerald-500/10",
    ring: "ring-emerald-500/30",
  },
  {
    id: "rule",
    label: "规则模式",
    sublabel: "仅匹配域名规则的请求走代理",
    icon: Shield,
    color: "text-blue-400",
    bg: "bg-blue-500/10",
    ring: "ring-blue-500/30",
  },
  {
    id: "direct",
    label: "直连模式",
    sublabel: "关闭系统代理，直连网络",
    icon: Unplug,
    color: "text-surface-400",
    bg: "bg-surface-800/50",
    ring: "ring-surface-600/30",
  },
];

export default function ProxyControl() {
  const proxyStatus = useAppStore((s) => s.proxyStatus);
  const whistleStatus = useAppStore((s) => s.whistleStatus);
  const setProxyMode = useAppStore((s) => s.setProxyMode);
  const refreshProxyStatus = useAppStore((s) => s.refreshProxyStatus);
  const startWhistle = useAppStore((s) => s.startWhistle);
  const error = useAppStore((s) => s.error);
  const [switching, setSwitching] = useState(false);
  const [localError, setLocalError] = useState<string | null>(null);

  useEffect(() => {
    refreshProxyStatus();
  }, []);

  const currentMode = (proxyStatus?.mode || "direct") as ProxyMode;
  const whistleAlive = whistleStatus?.running && whistleStatus?.uptime_check;

  const handleModeSwitch = async (mode: ProxyMode) => {
    if (mode === currentMode || switching) return;

    setLocalError(null);
    setSwitching(true);

    try {
      if (mode !== "direct" && !whistleAlive) {
        await startWhistle();
      }
      await setProxyMode(mode);
    } catch (e) {
      setLocalError(String(e));
    } finally {
      setSwitching(false);
    }
  };

  return (
    <div className="animate-fade-in">
      <div className="sticky top-0 z-10 bg-[var(--bg-main)] px-6 pt-6 pb-3 border-b border-[var(--border-color)]">
        <div className="max-w-5xl mx-auto">
          <h1 className="text-2xl font-bold themed-text tracking-tight">代理控制</h1>
          <p className="text-sm themed-text-muted mt-1">切换系统代理模式，管理网络流量走向</p>
        </div>
      </div>

      <div className="px-6 pb-6 pt-4 space-y-6 max-w-5xl mx-auto">
        {/* Current status banner */}
        <div
          className={`glass-panel p-4 flex items-center gap-4 border-l-[3px] ${
            currentMode === "global"
              ? "border-l-emerald-500"
              : currentMode === "rule"
                ? "border-l-blue-500"
                : "border-l-surface-600"
          }`}
        >
          <div className="flex-1">
            <div className="text-xs themed-text-muted mb-1">当前模式</div>
            <div className="text-lg font-semibold themed-text">
              {modes.find((m) => m.id === currentMode)?.label}
            </div>
          </div>
          {whistleAlive && (
            <div className="text-right">
              <div className="text-xs themed-text-muted">Whistle</div>
              <div className="text-sm font-mono text-accent-400">
                {proxyStatus?.host}:{proxyStatus?.port}
              </div>
            </div>
          )}
        </div>

        {!whistleAlive && currentMode !== "direct" && (
          <div className="glass-panel p-3 border-l-[3px] border-l-warning-500 flex items-center gap-3">
            <span className="status-dot status-dot--warning" />
            <span className="text-xs text-warning-400">
              Whistle 未运行，代理可能无法正常工作。切换模式时将自动启动。
            </span>
          </div>
        )}

        {/* Mode cards */}
        <div className="space-y-3">
          {modes.map((mode) => {
            const Icon = mode.icon;
            const isActive = currentMode === mode.id;
            return (
              <button
                key={mode.id}
                onClick={() => handleModeSwitch(mode.id)}
                disabled={switching}
                className={`w-full p-5 rounded-xl border text-left transition-all duration-300 ${
                  isActive
                    ? `${mode.bg} border-transparent ring-2 ${mode.ring}`
                    : "bg-surface-900/30 border-surface-800/50 hover:bg-surface-900/60 hover:border-surface-700/50"
                } ${switching ? "opacity-60 cursor-wait" : "active:scale-[0.99]"}`}
              >
                <div className="flex items-center gap-4">
                  <div
                    className={`w-12 h-12 rounded-xl flex items-center justify-center ${
                      isActive ? mode.bg : "bg-surface-800/70"
                    }`}
                  >
                    <Icon size={22} className={isActive ? mode.color : "text-surface-500"} />
                  </div>
                  <div className="flex-1">
                    <div
                      className={`text-base font-semibold ${
                        isActive ? "themed-text" : "themed-text-secondary"
                      }`}
                    >
                      {mode.label}
                    </div>
                    <div className="text-xs themed-text-muted mt-0.5">{mode.sublabel}</div>
                  </div>
                  {isActive && (
                    <div className="flex items-center gap-1.5">
                      <span
                        className={`w-2 h-2 rounded-full ${
                          mode.id === "global"
                            ? "bg-emerald-400"
                            : mode.id === "rule"
                              ? "bg-blue-400"
                              : "bg-surface-400"
                        }`}
                      />
                      <span className={`text-xs font-medium ${mode.color}`}>已启用</span>
                    </div>
                  )}
                </div>
              </button>
            );
          })}
        </div>

        {(localError || (error && error.includes("代理"))) && (
          <div className="glass-panel p-3 border-l-[3px] border-l-danger-500 flex items-start gap-3">
            <AlertCircle size={16} className="text-danger-400 shrink-0 mt-0.5" />
            <div>
              <div className="text-xs text-danger-400 font-medium mb-1">切换失败</div>
              <div className="text-xs text-surface-400">{localError || error}</div>
            </div>
          </div>
        )}

        <div className="text-xs themed-text-muted leading-relaxed">
          <p>
            <strong className="text-surface-500">全局模式</strong>
            ：设置系统 HTTP/HTTPS 代理指向 Whistle。所有浏览器和支持系统代理的应用流量都将经过
            Whistle。
          </p>
          <p className="mt-1.5">
            <strong className="text-surface-500">规则模式</strong>
            ：通过 PAC 文件，仅将匹配规则的域名流量转发至
            Whistle，其余直连。需先在规则页面配置域名。
          </p>
          <p className="mt-1.5">
            <strong className="text-surface-500">直连模式</strong>
            ：清除系统代理设置。Whistle 保持运行但不接管流量。
          </p>
        </div>
      </div>
    </div>
  );
}
