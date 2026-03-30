import { useEffect, useState } from "react";
import {
  Globe,
  ArrowUpRight,
  Server,
  ListFilter,
  RefreshCw,
  Wrench,
  CheckCircle2,
  XCircle,
  Loader2,
} from "lucide-react";
import { invoke } from "@tauri-apps/api/core";
import { useAppStore } from "../../stores/appStore";
import type { Page } from "../../types";

interface RepairStep {
  name: string;
  success: boolean;
  message: string;
}

interface RepairResult {
  success: boolean;
  steps: RepairStep[];
}

interface DashboardProps {
  onNavigate: (page: Page) => void;
}

export default function Dashboard({ onNavigate }: DashboardProps) {
  const config = useAppStore((s) => s.config);
  const whistleStatus = useAppStore((s) => s.whistleStatus);
  const proxyStatus = useAppStore((s) => s.proxyStatus);
  const refreshWhistleStatus = useAppStore((s) => s.refreshWhistleStatus);
  const refreshProxyStatus = useAppStore((s) => s.refreshProxyStatus);

  useEffect(() => {
    const initialDelay = setTimeout(() => {
      refreshWhistleStatus().catch(() => { });
      refreshProxyStatus().catch(() => { });
    }, 1500);

    const interval = setInterval(() => {
      if (document.visibilityState === "visible") {
        refreshWhistleStatus().catch(() => { });
        refreshProxyStatus().catch(() => { });
      }
    }, 8000);

    return () => {
      clearTimeout(initialDelay);
      clearInterval(interval);
    };
  }, [refreshWhistleStatus, refreshProxyStatus]);

  const [repairing, setRepairing] = useState(false);
  const [repairResult, setRepairResult] = useState<RepairResult | null>(null);

  const handleRepairNetwork = async () => {
    setRepairing(true);
    setRepairResult(null);
    try {
      const result = await invoke<RepairResult>("cmd_repair_network");
      setRepairResult(result);
      refreshProxyStatus();
    } catch (e) {
      setRepairResult({
        success: false,
        steps: [{ name: "网络诊断", success: false, message: String(e) }],
      });
    } finally {
      setRepairing(false);
    }
  };

  const proxyModeLabel =
    proxyStatus?.mode === "global"
      ? "全局模式"
      : proxyStatus?.mode === "rule"
        ? "规则模式"
        : "直连模式";

  const proxyActive = proxyStatus?.mode !== "direct" && proxyStatus?.enabled;
  const whistleAlive = whistleStatus?.running && whistleStatus?.uptime_check;
  const activeRulesCount =
    config?.profiles
      ?.find((p) => p.id === config?.active_profile_id)
      ?.rules?.filter((r) => r.enabled)?.length ?? 0;

  return (
    <div className="animate-fade-in">
      <div className="sticky top-0 z-10 bg-[var(--bg-main)] px-6 pt-6 pb-3 border-b border-[var(--border-color)]">
        <div className="flex items-center justify-between max-w-5xl mx-auto">
          <div>
            <h1 className="text-2xl font-bold themed-text tracking-tight">仪表盘</h1>
            <p className="text-sm themed-text-muted mt-1">WhistleBox 运行状态概览</p>
          </div>
          <button
            onClick={() => {
              refreshWhistleStatus();
              refreshProxyStatus();
            }}
            className="btn-secondary flex items-center gap-2 text-xs"
          >
            <RefreshCw size={14} />
            刷新
          </button>
        </div>
      </div>

      <div className="px-6 pb-6 pt-4 space-y-6 max-w-5xl mx-auto">
        {/* Status Cards */}
        <div className="grid grid-cols-3 gap-4">
          <button
            onClick={() => onNavigate("proxy")}
            className="card group text-left flex flex-col justify-start"
          >
            <div className="flex items-start justify-between mb-4">
              <div
                className={`w-10 h-10 rounded-xl flex items-center justify-center ${proxyStatus?.mode === "global"
                    ? "bg-emerald-500/15 text-emerald-400"
                    : proxyStatus?.mode === "rule"
                      ? "bg-blue-500/15 text-blue-400"
                      : "bg-surface-800 text-surface-500"
                  }`}
              >
                <Globe size={20} />
              </div>
              <ArrowUpRight
                size={16}
                className="text-surface-600 group-hover:text-surface-400 transition-colors"
              />
            </div>
            <div className="text-xs text-surface-500 mb-1">代理状态</div>
            <div className="flex items-center gap-2">
              <span
                className={`w-2 h-2 rounded-full ${proxyStatus?.mode === "global"
                    ? "bg-emerald-400"
                    : proxyStatus?.mode === "rule"
                      ? "bg-blue-400"
                      : "bg-surface-500"
                  }`}
              />
              <span className="text-lg font-semibold text-surface-200">{proxyModeLabel}</span>
            </div>
            {proxyActive && (
              <div className="text-xs text-surface-500 mt-2 font-mono">
                {proxyStatus?.host}:{proxyStatus?.port}
              </div>
            )}
          </button>

          <button
            onClick={() => onNavigate("whistle")}
            className="card group text-left flex flex-col justify-start"
          >
            <div className="flex items-start justify-between mb-4">
              <div
                className={`w-10 h-10 rounded-xl flex items-center justify-center ${whistleAlive
                    ? "bg-accent-500/15 text-accent-400"
                    : "bg-surface-800 text-surface-500"
                  }`}
              >
                <Server size={20} />
              </div>
              <ArrowUpRight
                size={16}
                className="text-surface-600 group-hover:text-surface-400 transition-colors"
              />
            </div>
            <div className="text-xs text-surface-500 mb-1">Whistle</div>
            <div className="flex items-center gap-2">
              <span
                className={`status-dot ${whistleAlive
                    ? "status-dot--active"
                    : whistleStatus?.running
                      ? "status-dot--warning"
                      : "status-dot--inactive"
                  }`}
              />
              <span className="text-lg font-semibold text-surface-200">
                {config?.whistle?.mode === "embedded"
                  ? whistleAlive
                    ? "内置运行中"
                    : whistleStatus?.running
                      ? "内置异常"
                      : "内置未启动"
                  : whistleAlive
                    ? "外部已连接"
                    : "外部未连接"}
              </span>
            </div>
            {config?.whistle?.mode === "embedded" && whistleStatus?.pid ? (
              <div className="text-xs text-surface-500 mt-2 font-mono">
                PID: {whistleStatus.pid}
              </div>
            ) : config?.whistle?.mode !== "embedded" ? (
              <div className="text-xs text-surface-500 mt-2 font-mono">
                {config?.app_settings?.external_host}:{config?.app_settings?.external_port}
              </div>
            ) : null}
          </button>

          <button
            onClick={() => onNavigate("rules")}
            className="card group text-left flex flex-col justify-start"
          >
            <div className="flex items-start justify-between mb-4">
              <div className="w-10 h-10 rounded-xl flex items-center justify-center bg-surface-800 text-surface-400">
                <ListFilter size={20} />
              </div>
              <ArrowUpRight
                size={16}
                className="text-surface-600 group-hover:text-surface-400 transition-colors"
              />
            </div>
            <div className="text-xs text-surface-500 mb-1">活跃规则</div>
            <div className="flex items-center gap-2">
              <span className="text-lg font-semibold text-surface-200">{activeRulesCount}</span>
              <span className="text-xs text-surface-500">条规则</span>
            </div>
            <div className="text-xs text-surface-500 mt-2">
              配置: {config?.profiles?.find((p) => p.id === config?.active_profile_id)?.name ?? "-"}
            </div>
          </button>
        </div>

        {/* Connection Info */}
        {config && (
          <div>
            <h2 className="text-sm font-semibold themed-text-secondary mb-3 uppercase tracking-wider">
              连接信息
            </h2>
            <div className="glass-panel p-4">
              <div className="grid grid-cols-2 gap-4">
                <InfoRow
                  label="Whistle 模式"
                  value={config.whistle.mode === "embedded" ? "内置模式" : "外部模式"}
                />
                {config.whistle.mode === "embedded" ? (
                  <>
                    <InfoRow
                      label="Whistle 地址"
                      value={`${config.whistle.host}:${config.whistle.port}`}
                      mono
                    />
                    {config.whistle.socks_port ? (
                      <InfoRow
                        label="SOCKS 代理"
                        value={`${config.whistle.host}:${config.whistle.socks_port}`}
                        mono
                      />
                    ) : null}
                    <InfoRow label="认证代理" value={`127.0.0.1:${config.auth_proxy_port}`} mono />
                    <InfoRow label="PAC 服务" value={`127.0.0.1:${config.pac_server_port}`} mono />
                    <InfoRow label="请求超时" value={`${config.whistle.timeout || 60} 秒`} />
                    <InfoRow
                      label="HTTPS 拦截"
                      value={config.whistle.intercept_https ? "已启用" : "未启用"}
                    />
                  </>
                ) : (
                  <>
                    <InfoRow
                      label="外部地址"
                      value={`${config.app_settings.external_host}:${config.app_settings.external_port}`}
                      mono
                    />
                    <InfoRow label="连接状态" value={whistleAlive ? "已连接" : "未连接"} />
                    <InfoRow label="认证代理" value={`127.0.0.1:${config.auth_proxy_port}`} mono />
                  </>
                )}
              </div>
            </div>
          </div>
        )}

        {/* Network Repair */}
        <div>
          <h2 className="text-sm font-semibold themed-text-secondary mb-3 uppercase tracking-wider">
            实用工具
          </h2>
          <div className="glass-panel p-4">
            <div className={`flex items-center justify-between ${repairResult ? "mb-3" : ""}`}>
              <div>
                <div className="text-sm font-medium themed-text">修复网络连接</div>
                <div className="text-xs themed-text-muted mt-0.5">
                  清除系统代理、PAC 配置、刷新 DNS 缓存，恢复正常网络访问
                </div>
              </div>
              <button
                onClick={handleRepairNetwork}
                disabled={repairing}
                className="btn-secondary flex items-center gap-2 text-xs shrink-0"
              >
                {repairing ? <Loader2 size={14} className="animate-spin" /> : <Wrench size={14} />}
                {repairing ? "修复中..." : "一键修复"}
              </button>
            </div>
            {repairResult && (
              <div className="space-y-2 pt-3 border-t border-[var(--border-color)]">
                {repairResult.steps.map((step, i) => (
                  <div key={i} className="flex items-center gap-2 text-xs">
                    {step.success ? (
                      <CheckCircle2 size={13} className="text-accent-400 shrink-0" />
                    ) : (
                      <XCircle size={13} className="text-red-400 shrink-0" />
                    )}
                    <span className="themed-text-secondary">{step.name}</span>
                    <span className="themed-text-muted ml-auto">{step.message}</span>
                  </div>
                ))}
              </div>
            )}
          </div>
        </div>
      </div>
    </div>
  );
}

function InfoRow({ label, value, mono }: { label: string; value: string; mono?: boolean }) {
  return (
    <div className="flex items-center justify-between py-1.5">
      <span className="text-xs text-surface-500">{label}</span>
      <span className={`text-xs text-surface-300 ${mono ? "font-mono" : ""}`}>{value}</span>
    </div>
  );
}
