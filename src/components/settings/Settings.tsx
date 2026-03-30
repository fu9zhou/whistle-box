import {
  Server,
  Key,
  Plug,
  CheckCircle2,
  XCircle,
  Loader2,
  Network,
  Settings2,
  Monitor,
  RotateCcw,
  RefreshCw,
  Download,
  Eye,
  EyeOff,
} from "lucide-react";
import { useSettingsForm } from "./useSettingsForm";
import { Section, Field, Label, Hint, Toggle, ModeButton } from "./SettingsUI";

export default function Settings() {
  const s = useSettingsForm();

  return (
    <div className="animate-fade-in">
      <div className="sticky top-0 z-10 bg-[var(--bg-main)] px-6 pt-6 pb-3 border-b border-[var(--border-color)]">
        <div className="flex items-center justify-between max-w-5xl mx-auto">
          <div>
            <h1 className="text-2xl font-bold themed-text tracking-tight">设置</h1>
            <p className="text-sm themed-text-muted mt-1">Whistle 连接配置与应用设置</p>
          </div>
          <div className="flex items-center gap-2">
            {s.whistleRestarting && (
              <button
                disabled
                className="flex items-center gap-2 text-sm font-medium px-4 py-2 rounded-lg btn-primary opacity-70 cursor-not-allowed animate-fade-in"
              >
                <Loader2 size={14} className="animate-spin" />
                重启中...
              </button>
            )}
            {!s.whistleRestarting &&
              s.needsWhistleRestart &&
              s.form.whistleMode === "embedded" &&
              s.embeddedRunning && (
                <button
                  onClick={s.handleRestartWhistle}
                  className="flex items-center gap-2 text-sm font-medium px-4 py-2 rounded-lg btn-primary transition-all duration-200 active:scale-[0.97] animate-fade-in"
                >
                  <RefreshCw size={14} />
                  重启 Whistle
                </button>
              )}
            {!s.whistleRestarting && s.form.whistleMode === "embedded" && !s.embeddedRunning && (
              <button
                onClick={s.handleStartWhistle}
                disabled={s.whistleStarting}
                className="flex items-center gap-2 text-sm font-medium px-4 py-2 rounded-lg btn-primary transition-all duration-200 active:scale-[0.97] animate-fade-in disabled:opacity-70 disabled:cursor-not-allowed"
              >
                {s.whistleStarting ? (
                  <Loader2 size={14} className="animate-spin" />
                ) : (
                  <Server size={14} />
                )}
                {s.whistleStarting ? "启动中..." : "启动 Whistle"}
              </button>
            )}
            {!s.isDefault && (
              <button
                onClick={() => s.setShowResetConfirm(true)}
                className="flex items-center gap-2 text-sm font-medium px-4 py-2 rounded-lg btn-secondary transition-all duration-200 active:scale-[0.97]"
              >
                <RotateCcw size={14} />
                恢复默认
              </button>
            )}
            {s.showResetConfirm && (
              <div
                className="fixed inset-0 z-50 flex items-center justify-center animate-fade-in"
                role="dialog"
                aria-modal="true"
                aria-label="恢复默认设置确认"
                tabIndex={-1}
                ref={(el) => el?.focus()}
                onClick={() => s.setShowResetConfirm(false)}
                onKeyDown={(e) => {
                  if (e.key === "Escape") s.setShowResetConfirm(false);
                }}
              >
                <div className="absolute inset-0 bg-black/50 backdrop-blur-sm" />
                <div
                  className="relative glass-panel w-80 p-0 overflow-hidden shadow-2xl"
                  style={{ animation: "dialog-pop 0.2s ease-out" }}
                  onClick={(e) => e.stopPropagation()}
                >
                  <div className="px-5 pt-5 pb-4">
                    <div className="flex items-center gap-2.5 mb-3">
                      <div className="w-8 h-8 rounded-lg bg-warning-500/15 flex items-center justify-center">
                        <RotateCcw size={15} className="text-warning-400" />
                      </div>
                      <h3 className="text-sm font-semibold themed-text">恢复默认设置</h3>
                    </div>
                    <p className="text-xs themed-text-muted leading-relaxed">
                      所有设置项将恢复为默认值，已配置的用户名和密码不受影响。此操作不可撤销。
                    </p>
                  </div>
                  <div className="flex border-t border-[var(--border-color)]">
                    <button
                      onClick={() => s.setShowResetConfirm(false)}
                      className="flex-1 py-2.5 text-xs font-medium themed-text-secondary hover:themed-text hover:bg-[var(--bg-tertiary)] transition-colors border-r border-[var(--border-color)]"
                    >
                      取消
                    </button>
                    <button
                      onClick={s.handleResetForm}
                      className="flex-1 py-2.5 text-xs font-medium text-warning-400 hover:bg-warning-500/10 transition-colors"
                    >
                      确定恢复
                    </button>
                  </div>
                </div>
              </div>
            )}
          </div>
        </div>
      </div>

      <div className="px-6 pb-6 pt-4 space-y-6 max-w-5xl mx-auto lg:columns-2 lg:gap-6">
        {/* Whistle Connection */}
        <Section
          icon={<Server size={16} />}
          title="Whistle 连接"
          action={
            s.form.whistleMode === "external" ? (
              <div className="flex items-center gap-1.5">
                <button
                  onClick={s.handleTestConnection}
                  disabled={s.testing}
                  className="text-[11px] flex items-center gap-1 text-surface-500 hover:text-accent-400 transition-colors"
                >
                  <Plug size={10} />
                  测试连接
                </button>
                <span className="w-3 h-3 flex items-center justify-center">
                  {s.testing ? (
                    <Loader2 size={10} className="animate-spin text-surface-400" />
                  ) : s.testResult !== null ? (
                    <span
                      className={`text-[11px] ${s.testResult ? "text-accent-400" : "text-danger-400"}`}
                    >
                      {s.testResult ? "✓" : "✗"}
                    </span>
                  ) : null}
                </span>
              </div>
            ) : undefined
          }
        >
          <div>
            <Label>运行模式</Label>
            <div className="flex gap-2">
              <ModeButton
                active={s.form.whistleMode === "embedded"}
                onClick={s.handleSwitchToEmbedded}
                color="accent"
                label="内置模式"
                sublabel="使用内置 Whistle，自动管理"
              />
              <ModeButton
                active={s.form.whistleMode === "external"}
                onClick={s.handleSwitchToExternal}
                color="blue"
                label="外部模式"
                sublabel="连接外部 Whistle 实例"
              />
            </div>
          </div>

          <div className="grid grid-cols-2 gap-3">
            <Field label="地址">
              <input
                type="text"
                value={s.form.whistleMode === "embedded" ? s.form.whistleHost : s.form.externalHost}
                onChange={(e) =>
                  s.form.whistleMode === "embedded"
                    ? s.updateForm({ whistleHost: e.target.value })
                    : s.updateForm({ externalHost: e.target.value })
                }
                className="input-field w-full font-mono text-sm"
              />
            </Field>
            <Field label={s.form.whistleMode === "embedded" ? "端口 (-p)" : "端口"}>
              <input
                type="number"
                value={s.form.whistleMode === "embedded" ? s.form.whistlePort : s.form.externalPort}
                onChange={(e) =>
                  s.form.whistleMode === "embedded"
                    ? s.updateForm({ whistlePort: parseInt(e.target.value, 10) || 18899 })
                    : s.updateForm({ externalPort: parseInt(e.target.value, 10) || 8899 })
                }
                className="input-field w-full font-mono text-sm"
              />
            </Field>
          </div>
        </Section>

        {/* Authentication */}
        <Section icon={<Key size={16} />} title="认证信息">
          <div className="grid grid-cols-2 gap-3">
            <Field label={s.form.whistleMode === "embedded" ? "用户名 (-n)" : "用户名"}>
              <input
                type="text"
                value={
                  s.form.whistleMode === "embedded" ? s.form.username : s.form.externalUsername
                }
                onChange={(e) =>
                  s.form.whistleMode === "embedded"
                    ? s.updateForm({ username: e.target.value })
                    : s.updateForm({ externalUsername: e.target.value })
                }
                placeholder="whistle 用户名"
                className="input-field w-full font-mono text-sm"
              />
            </Field>
            <Field label={s.form.whistleMode === "embedded" ? "密码 (-w)" : "密码"}>
              <div className="relative">
                <input
                  type={s.showPassword ? "text" : "password"}
                  value={
                    s.form.whistleMode === "embedded" ? s.form.password : s.form.externalPassword
                  }
                  onChange={(e) =>
                    s.form.whistleMode === "embedded"
                      ? s.updateForm({ password: e.target.value })
                      : s.updateForm({ externalPassword: e.target.value })
                  }
                  placeholder="whistle 密码"
                  className="input-field w-full text-sm font-mono pr-9"
                />
                <button
                  type="button"
                  onClick={() => s.setShowPassword((v) => !v)}
                  className="absolute right-2.5 top-1/2 -translate-y-1/2 text-surface-500 hover:text-surface-300 transition-colors"
                >
                  {s.showPassword ? <EyeOff size={14} /> : <Eye size={14} />}
                </button>
              </div>
            </Field>
          </div>
          <Hint>
            {s.form.whistleMode === "embedded"
              ? "内置模式下，用于启动 Whistle 并自动认证。"
              : "外部模式下，用于连接远程 Whistle 实例的认证。"}
          </Hint>
        </Section>

        {/* Whistle Launch Parameters - embedded only */}
        {s.form.whistleMode === "embedded" && (
          <Section icon={<Network size={16} />} title="Whistle 启动参数">
            <Field label="上游代理" hint="通过上游代理转发所有 Whistle 出站流量，格式：host:port">
              <input
                type="text"
                value={s.form.upstreamProxy}
                onChange={(e) => s.updateForm({ upstreamProxy: e.target.value })}
                placeholder="例如 proxy.company.com:3128"
                className="input-field w-full font-mono text-sm"
              />
            </Field>

            <Field label="存储路径 (-D)">
              <input
                type="text"
                value={s.form.storagePath || "~/.WhistleBoxData"}
                onChange={(e) =>
                  s.updateForm({
                    storagePath: e.target.value === "~/.WhistleBoxData" ? "" : e.target.value,
                  })
                }
                className="input-field w-full font-mono text-sm"
              />
            </Field>

            <div className="grid grid-cols-2 gap-3">
              <Field label="SOCKS5 代理端口 (-S)">
                <input
                  type="number"
                  value={s.form.socksPort || ""}
                  onChange={(e) =>
                    s.updateForm({ socksPort: e.target.value ? parseInt(e.target.value, 10) : 0 })
                  }
                  placeholder="留空不启用"
                  className="input-field w-full font-mono text-sm"
                />
              </Field>
              <Field label="请求超时（秒）(-t)">
                <input
                  type="number"
                  value={s.form.timeout}
                  onChange={(e) => s.updateForm({ timeout: parseInt(e.target.value, 10) || 60 })}
                  className="input-field w-full font-mono text-sm"
                />
              </Field>
            </div>

            <div className="space-y-3 pt-1">
              <Toggle
                checked={s.form.interceptHttps}
                onChange={(v) => s.updateForm({ interceptHttps: v }, true)}
                label="拦截 HTTPS (-c)"
                sublabel="启用 HTTPS 抓包，需安装 Whistle CA 证书"
              />

              <div className="ml-12 pl-3 border-l-2 border-[var(--border-color)] space-y-2">
                <div className="text-[11px] themed-text-muted leading-relaxed">
                  {s.form.interceptHttps
                    ? "已启用 HTTPS 拦截，需安装根证书才能正常抓包。"
                    : "如需使用 HTTPS 拦截，请提前安装根证书。"}
                </div>
                <div className="flex items-center gap-2 flex-wrap">
                  <button
                    onClick={s.handleInstallCert}
                    disabled={s.certInstalling || s.certRemoving || !s.embeddedRunning}
                    className="flex items-center gap-1.5 text-[11px] font-medium px-2.5 py-1 rounded-md btn-primary transition-all duration-200 active:scale-[0.97] disabled:opacity-50 disabled:cursor-not-allowed"
                  >
                    {s.certInstalling ? (
                      <Loader2 size={11} className="animate-spin" />
                    ) : (
                      <Download size={11} />
                    )}
                    {s.certInstalling ? "安装中..." : s.certInstalled ? "重新安装证书" : "安装证书"}
                  </button>
                  {s.certInstalled && (
                    <button
                      onClick={s.handleUninstallCert}
                      disabled={s.certRemoving || s.certInstalling}
                      className="flex items-center gap-1.5 text-[11px] font-medium px-2.5 py-1 rounded-md btn-secondary transition-all duration-200 active:scale-[0.97] disabled:opacity-50 disabled:cursor-not-allowed"
                    >
                      {s.certRemoving ? (
                        <Loader2 size={11} className="animate-spin" />
                      ) : (
                        <XCircle size={11} />
                      )}
                      {s.certRemoving ? "移除中..." : "移除证书"}
                    </button>
                  )}
                  {!s.embeddedRunning && (
                    <span className="text-[10px] themed-text-muted">需先启动 Whistle</span>
                  )}
                </div>
                {s.certResult && (
                  <div
                    className={`flex items-center gap-1.5 text-[11px] ${s.certResult.ok ? "text-emerald-400" : "text-danger-400"}`}
                  >
                    {s.certResult.ok ? <CheckCircle2 size={11} /> : <XCircle size={11} />}
                    {s.certResult.msg}
                  </div>
                )}
              </div>
            </div>
          </Section>
        )}

        {/* App & Ports */}
        <Section
          icon={<Settings2 size={16} />}
          title="应用设置"
          action={
            s.needsAppRestart ? (
              <button
                onClick={async () => {
                  try {
                    const { relaunch } = await import("@tauri-apps/plugin-process");
                    await relaunch();
                  } catch (e) {
                    console.error("Failed to relaunch:", e);
                  }
                }}
                className="text-[11px] flex items-center gap-1 text-warning-400 hover:text-warning-300 transition-colors animate-fade-in"
              >
                <RotateCcw size={10} />
                重启应用
              </button>
            ) : undefined
          }
        >
          <div className="grid grid-cols-2 gap-3">
            <Field label="PAC 服务端口">
              <input
                type="number"
                value={s.form.pacServerPort}
                onChange={(e) =>
                  s.updateForm({ pacServerPort: parseInt(e.target.value, 10) || 18901 })
                }
                className="input-field w-full font-mono text-sm"
              />
            </Field>
            <Field label="认证代理端口">
              <input
                type="number"
                value={s.form.authProxyPort}
                onChange={(e) =>
                  s.updateForm({ authProxyPort: parseInt(e.target.value, 10) || 18900 })
                }
                className="input-field w-full font-mono text-sm"
              />
            </Field>
          </div>

          <Field label="代理例外" hint="系统代理不走代理的地址，分号分隔，支持通配符">
            <input
              type="text"
              value={s.form.proxyBypass}
              onChange={(e) => s.updateForm({ proxyBypass: e.target.value })}
              className="input-field w-full font-mono text-sm text-[12px]"
            />
          </Field>

          <div className="space-y-3 pt-2">
            <Toggle
              checked={s.autoStartEnabled}
              onChange={s.handleToggleAutoStart}
              label="开机自启动"
              sublabel="系统启动时自动运行 WhistleBox"
            />
            <Toggle
              checked={s.form.localAuthBypass}
              onChange={(v) => s.updateForm({ localAuthBypass: v }, true)}
              label="本地免认证"
              sublabel="允许本机浏览器通过免认证端口直接访问 Whistle 界面"
            />
            {s.form.whistleMode === "embedded" && (
              <Toggle
                checked={s.form.autoStartWhistle}
                onChange={(v) => s.updateForm({ autoStartWhistle: v }, true)}
                label="自动启动 Whistle"
                sublabel="应用启动时自动启动内置 Whistle"
              />
            )}
            <Toggle
              checked={s.form.autoStartProxy}
              onChange={(v) => s.updateForm({ autoStartProxy: v }, true)}
              label="自动启用代理"
              sublabel="Whistle 就绪后自动恢复上次的代理模式"
            />
          </div>
        </Section>

        {/* Tray Settings */}
        <Section icon={<Monitor size={16} />} title="托盘设置">
          <Toggle
            checked={s.form.minimizeToTray}
            onChange={(v) => s.updateForm({ minimizeToTray: v }, true)}
            label="关闭时最小化到托盘"
            sublabel="关闭窗口时隐藏到系统托盘，而不是退出应用"
          />
          <Field label="托盘左键点击行为">
            <select
              value={s.form.trayClickAction}
              onChange={(e) =>
                s.updateForm(
                  {
                    trayClickAction: e.target.value as
                      | "show_window"
                      | "toggle_direct_rule"
                      | "toggle_direct_global"
                      | "cycle",
                  },
                  true,
                )
              }
              className="input-field w-full text-sm"
            >
              <option value="show_window">显示 / 隐藏主窗口</option>
              <option value="toggle_direct_rule">切换: 直连 ↔ 规则代理</option>
              <option value="toggle_direct_global">切换: 直连 ↔ 全局代理</option>
              <option value="cycle">循环: 直连 → 规则 → 全局</option>
            </select>
          </Field>
        </Section>
      </div>

      {/* Save Toast */}
      {s.saveToast && (
        <div className="fixed bottom-6 right-6 z-50 animate-fade-in">
          <div
            className={`flex items-center gap-2 px-4 py-2.5 rounded-lg text-sm font-medium shadow-lg ${(s.needsWhistleRestart && s.embeddedRunning) || s.needsAppRestart
                ? "bg-warning-500/90 text-white"
                : "bg-emerald-600/90 text-white"
              }`}
          >
            <CheckCircle2 size={14} />
            {s.saveToast}
          </div>
        </div>
      )}
    </div>
  );
}
