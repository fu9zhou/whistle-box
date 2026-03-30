import { useState, useEffect, useCallback } from "react";
import {
  Server,
  Key,
  ArrowRight,
  ArrowLeft,
  Check,
  Zap,
  Shield,
  Download,
  Loader2,
  CheckCircle2,
  XCircle,
  RefreshCw,
  Eye,
  EyeOff,
} from "lucide-react";
import { invoke } from "@tauri-apps/api/core";
import { useAppStore, type AppConfig } from "../../stores/appStore";
import { buildFallbackConfig, DEFAULT_WHISTLE_PORT, DEFAULT_EXTERNAL_PORT } from "../../defaults";

interface SetupWizardProps {
  onComplete: () => void;
}

export default function SetupWizard({ onComplete }: SetupWizardProps) {
  const config = useAppStore((s) => s.config);
  const startWhistle = useAppStore((s) => s.startWhistle);
  const stopWhistle = useAppStore((s) => s.stopWhistle);
  const startAuthProxy = useAppStore((s) => s.startAuthProxy);
  const [step, setStep] = useState(0);
  const [mode, setMode] = useState<"embedded" | "external">("embedded");
  const [username, setUsername] = useState("");
  const [password, setPassword] = useState("");
  const [host, setHost] = useState("127.0.0.1");
  const [port, setPort] = useState(
    mode === "embedded" ? DEFAULT_WHISTLE_PORT : DEFAULT_EXTERNAL_PORT,
  );
  const [finishing, setFinishing] = useState(false);
  const [certInstalling, setCertInstalling] = useState(false);
  const [certResult, setCertResult] = useState<{ ok: boolean; msg: string } | null>(null);
  const [certAlreadyInstalled, setCertAlreadyInstalled] = useState<boolean | null>(null);
  const [showPassword, setShowPassword] = useState(false);

  const totalSteps = mode === "embedded" ? 3 : 2;

  const checkCertStatus = useCallback(async () => {
    try {
      const installed = await invoke<boolean>("cmd_check_cert_installed");
      setCertAlreadyInstalled(installed);
    } catch {
      setCertAlreadyInstalled(null);
    }
  }, []);

  useEffect(() => {
    checkCertStatus();
  }, [checkCertStatus]);

  const handleModeChange = (m: "embedded" | "external") => {
    setMode(m);
    setPort(m === "embedded" ? DEFAULT_WHISTLE_PORT : DEFAULT_EXTERNAL_PORT);
  };

  const [configSaved, setConfigSaved] = useState(false);
  const [saveError, setSaveError] = useState<string | null>(null);
  const loadConfig = useAppStore((s) => s.loadConfig);

  const saveAndStart = async (markComplete: boolean = false): Promise<boolean> => {
    if (!config) return false;
    if (configSaved && !markComplete) return true;
    setFinishing(true);
    setSaveError(null);

    const newConfig: AppConfig = {
      ...config,
      whistle: {
        ...config.whistle,
        mode,
        host: mode === "embedded" ? host : config.whistle.host,
        port: mode === "embedded" ? port : config.whistle.port,
        username: mode === "embedded" ? username : config.whistle.username,
        password: mode === "embedded" ? password : config.whistle.password,
        intercept_https: mode === "embedded" ? true : config.whistle.intercept_https,
      },
      app_settings: {
        ...config.app_settings,
        external_host: mode === "external" ? host : config.app_settings.external_host,
        external_port: mode === "external" ? port : config.app_settings.external_port,
        external_username:
          mode === "external" ? username : (config.app_settings.external_username ?? ""),
        external_password:
          mode === "external" ? password : (config.app_settings.external_password ?? ""),
      },
      setup_completed: markComplete,
    };

    try {
      await invoke("cmd_save_config", { config: newConfig });
    } catch (e) {
      setSaveError(String(e));
      setFinishing(false);
      return false;
    }

    if (markComplete) {
      await loadConfig();
    }

    if (mode === "embedded" && !configSaved) {
      try {
        await stopWhistle();
        await new Promise((r) => setTimeout(r, 1000));
        await startWhistle();
        await startAuthProxy();
      } catch (e) {
        console.error("Failed to start whistle/auth:", e);
      }
    } else if (mode === "external" && !configSaved) {
      try {
        await startAuthProxy();
      } catch (e) {
        console.error("Failed to start auth proxy:", e);
      }
    }

    setConfigSaved(true);
    setFinishing(false);
    return true;
  };

  const handleFinish = async () => {
    if (finishing) return;
    setSaveError(null);
    if (!config) {
      const fallbackConfig: AppConfig = buildFallbackConfig();
      setFinishing(true);
      try {
        await invoke("cmd_save_config", { config: fallbackConfig });
        await loadConfig();
      } catch (e) {
        setSaveError(String(e));
        setFinishing(false);
        return;
      }
      setFinishing(false);
      onComplete();
      return;
    }
    const ok = await saveAndStart(true);
    if (ok) onComplete();
  };

  return (
    <div className="h-screen w-screen flex items-center justify-center themed-main">
      <div className="w-full max-w-lg mx-4">
        <div className="text-center mb-8">
          <div className="w-16 h-16 rounded-2xl bg-accent-600/20 flex items-center justify-center mx-auto mb-4">
            <Zap size={32} className="text-accent-400" />
          </div>
          <h1 className="text-2xl font-bold themed-text">欢迎使用 WhistleBox</h1>
          <p className="text-sm themed-text-muted mt-2">快速配置，开始使用</p>
        </div>

        <div className="flex items-center justify-center gap-2 mb-6">
          {Array.from({ length: totalSteps }, (_, i) => (
            <div
              key={i}
              className={`h-1.5 rounded-full transition-all ${i === step
                  ? "w-8 bg-accent-500"
                  : i < step
                    ? "w-6 bg-accent-700"
                    : "w-6 bg-surface-700"
                }`}
            />
          ))}
        </div>

        <div className="glass-panel p-6 flex flex-col" style={{ minHeight: 288 }}>
          {step === 0 && (
            <div className="flex flex-col flex-1 animate-fade-in">
              <div className="space-y-5 flex-1">
                <div className="flex items-center gap-2 text-sm font-semibold themed-text-secondary">
                  <Server size={16} className="text-accent-400" />
                  选择 Whistle 模式
                </div>

                <div className="flex gap-3">
                  <button
                    onClick={() => handleModeChange("embedded")}
                    className={`flex-1 p-4 rounded-lg border text-left transition-all ${mode === "embedded"
                        ? "bg-accent-950/40 border-accent-700/30"
                        : "bg-surface-900/50 border-surface-800 hover:border-surface-700"
                      }`}
                  >
                    <div
                      className={`font-medium text-sm ${mode === "embedded" ? "text-accent-400" : "themed-text-secondary"}`}
                    >
                      内置模式
                    </div>
                    <div className="text-[11px] themed-text-muted mt-1">
                      内置 Whistle 实例，便捷管理
                    </div>
                  </button>
                  <button
                    onClick={() => handleModeChange("external")}
                    className={`flex-1 p-4 rounded-lg border text-left transition-all ${mode === "external"
                        ? "bg-blue-950/40 border-blue-700/30"
                        : "bg-surface-900/50 border-surface-800 hover:border-surface-700"
                      }`}
                  >
                    <div
                      className={`font-medium text-sm ${mode === "external" ? "text-blue-400" : "themed-text-secondary"}`}
                    >
                      外部模式
                    </div>
                    <div className="text-[11px] themed-text-muted mt-1">连接外部 Whistle 实例</div>
                  </button>
                </div>

                <div className="grid grid-cols-2 gap-3">
                  <div>
                    <label className="block text-xs themed-text-muted mb-1.5">
                      {mode === "embedded" ? "监听地址" : "外部地址"}
                    </label>
                    <input
                      type="text"
                      value={host}
                      onChange={(e) => setHost(e.target.value)}
                      className="input-field w-full font-mono text-sm"
                    />
                  </div>
                  <div>
                    <label className="block text-xs themed-text-muted mb-1.5">端口</label>
                    <input
                      type="number"
                      value={port}
                      onChange={(e) =>
                        setPort(
                          parseInt(e.target.value, 10) ||
                          (mode === "embedded" ? DEFAULT_WHISTLE_PORT : DEFAULT_EXTERNAL_PORT),
                        )
                      }
                      className="input-field w-full font-mono text-sm"
                    />
                  </div>
                </div>
              </div>

              <div className="flex justify-end mt-5">
                <button
                  onClick={() => setStep(1)}
                  className="btn-primary flex items-center gap-2 text-sm"
                >
                  下一步
                  <ArrowRight size={14} />
                </button>
              </div>
            </div>
          )}

          {step === 1 && (
            <div className="flex flex-col flex-1 animate-fade-in">
              <div className="space-y-5 flex-1">
                <div className="flex items-center gap-2 text-sm font-semibold themed-text-secondary">
                  <Key size={16} className="text-accent-400" />
                  认证信息
                </div>

                <div className="text-xs themed-text-muted">
                  {mode === "embedded"
                    ? "用于启动内置 Whistle 并自动认证，留空则不设密码"
                    : "用于连接外部 Whistle 实例的认证，留空则不使用认证"}
                </div>

                <div className="grid grid-cols-2 gap-3">
                  <div>
                    <label className="block text-xs themed-text-muted mb-1.5">用户名</label>
                    <input
                      type="text"
                      value={username}
                      onChange={(e) => setUsername(e.target.value)}
                      placeholder="可选"
                      className="input-field w-full font-mono text-sm"
                    />
                  </div>
                  <div>
                    <label className="block text-xs themed-text-muted mb-1.5">密码</label>
                    <div className="relative">
                      <input
                        type={showPassword ? "text" : "password"}
                        value={password}
                        onChange={(e) => setPassword(e.target.value)}
                        placeholder="可选"
                        className="input-field w-full font-mono text-sm pr-9"
                      />
                      <button
                        type="button"
                        onClick={() => setShowPassword((v) => !v)}
                        className="absolute right-2.5 top-1/2 -translate-y-1/2 text-surface-500 hover:text-surface-300 transition-colors"
                      >
                        {showPassword ? <EyeOff size={14} /> : <Eye size={14} />}
                      </button>
                    </div>
                  </div>
                </div>

                <div className="text-[11px] text-surface-300 bg-accent-950/30 border border-accent-800/20 rounded-lg px-3 py-2.5 text-center leading-relaxed">
                  设置用户名和密码可以防止他人未授权访问你的 Whistle 界面和代理流量
                  <br />
                  建议在共享网络环境下启用
                </div>
              </div>

              <div className="flex justify-between mt-5">
                <button
                  onClick={() => setStep(0)}
                  className="btn-secondary flex items-center gap-2 text-sm"
                >
                  <ArrowLeft size={14} />
                  上一步
                </button>
                {mode === "embedded" ? (
                  <button
                    onClick={() => setStep(2)}
                    className="btn-primary flex items-center gap-2 text-sm"
                  >
                    下一步
                    <ArrowRight size={14} />
                  </button>
                ) : (
                  <button
                    onClick={handleFinish}
                    disabled={finishing}
                    className="btn-primary flex items-center gap-2 text-sm"
                  >
                    {finishing ? "正在配置..." : "完成设置"}
                    <Check size={14} />
                  </button>
                )}
              </div>
            </div>
          )}

          {step === 2 && mode === "embedded" && (
            <div className="flex flex-col flex-1 animate-fade-in">
              <div className="space-y-5 flex-1">
                <div className="flex items-center gap-2 text-sm font-semibold themed-text-secondary">
                  <Shield size={16} className="text-accent-400" />
                  安装 HTTPS 根证书
                </div>

                {certAlreadyInstalled ? (
                  <>
                    <div className="flex items-center gap-2 text-sm text-emerald-400 bg-emerald-950/30 border border-emerald-800/20 rounded-lg px-4 py-3">
                      <CheckCircle2 size={16} />
                      已检测到 HTTPS 根证书已安装
                    </div>

                    <div className="text-xs themed-text-muted leading-relaxed">
                      如需更新证书，可点击下方按钮重新安装。
                    </div>

                    <div className="flex flex-col items-center gap-4 py-1">
                      <button
                        onClick={async () => {
                          setCertInstalling(true);
                          setCertResult(null);
                          try {
                            const saved = await saveAndStart();
                            if (!saved) {
                              setCertInstalling(false);
                              return;
                            }
                            await new Promise((r) => setTimeout(r, 2000));
                            const msg = await invoke<string>("cmd_install_cert");
                            setCertResult({ ok: true, msg });
                            setCertAlreadyInstalled(true);
                          } catch (e) {
                            setCertResult({ ok: false, msg: String(e) });
                          }
                          setCertInstalling(false);
                        }}
                        disabled={certInstalling}
                        className="btn-secondary flex items-center gap-2 text-xs px-4 py-2 disabled:opacity-50 disabled:cursor-not-allowed"
                      >
                        {certInstalling ? (
                          <Loader2 size={14} className="animate-spin" />
                        ) : (
                          <RefreshCw size={14} />
                        )}
                        {certInstalling ? "正在安装..." : "重新安装证书"}
                      </button>

                      {certResult && (
                        <div
                          className={`flex items-center gap-2 text-sm ${certResult.ok ? "text-emerald-400" : "text-danger-400"}`}
                        >
                          {certResult.ok ? <CheckCircle2 size={14} /> : <XCircle size={14} />}
                          {certResult.msg}
                        </div>
                      )}
                    </div>
                  </>
                ) : (
                  <>
                    <div className="text-xs themed-text-muted leading-relaxed">
                      内置模式默认启用 HTTPS 拦截，需安装 Whistle 的 CA 根证书才能正常抓包。
                      点击下方按钮将自动安装到系统受信任的根证书存储区。
                    </div>

                    <div className="flex flex-col items-center gap-4 py-3">
                      <button
                        onClick={async () => {
                          setCertInstalling(true);
                          setCertResult(null);
                          try {
                            const saved = await saveAndStart();
                            if (!saved) {
                              setCertInstalling(false);
                              return;
                            }
                            await new Promise((r) => setTimeout(r, 2000));
                            const msg = await invoke<string>("cmd_install_cert");
                            setCertResult({ ok: true, msg });
                            setCertAlreadyInstalled(true);
                          } catch (e) {
                            setCertResult({ ok: false, msg: String(e) });
                          }
                          setCertInstalling(false);
                        }}
                        disabled={certInstalling}
                        className="btn-primary flex items-center gap-2 text-sm px-6 py-2.5 disabled:opacity-50 disabled:cursor-not-allowed"
                      >
                        {certInstalling ? (
                          <Loader2 size={16} className="animate-spin" />
                        ) : (
                          <Download size={16} />
                        )}
                        {certInstalling ? "正在安装证书..." : "一键安装证书"}
                      </button>

                      {certResult && (
                        <div
                          className={`flex items-center gap-2 text-sm ${certResult.ok ? "text-emerald-400" : "text-danger-400"}`}
                        >
                          {certResult.ok ? <CheckCircle2 size={14} /> : <XCircle size={14} />}
                          {certResult.msg}
                        </div>
                      )}
                    </div>

                    <div className="text-[11px] text-surface-300 bg-accent-950/30 border border-accent-800/20 rounded-lg px-3 py-2.5 text-center leading-relaxed">
                      安装证书需要管理员权限，系统可能弹出确认提示
                      <br />
                      也可以稍后在设置中安装
                    </div>
                  </>
                )}
              </div>

              <div className="flex justify-between mt-5">
                <button
                  onClick={() => setStep(1)}
                  className="btn-secondary flex items-center gap-2 text-sm"
                >
                  <ArrowLeft size={14} />
                  上一步
                </button>
                <button
                  onClick={handleFinish}
                  disabled={finishing || certInstalling}
                  className="btn-primary flex items-center gap-2 text-sm"
                >
                  {finishing
                    ? "正在配置..."
                    : certInstalling
                      ? "证书安装中..."
                      : certResult || certAlreadyInstalled
                        ? "完成设置"
                        : "跳过，稍后安装"}
                  <Check size={14} />
                </button>
              </div>
            </div>
          )}
        </div>

        {saveError && (
          <div className="mt-3 mx-4 flex items-center gap-2 text-sm text-danger-400 bg-danger-950/30 border border-danger-800/20 rounded-lg px-4 py-2.5">
            <XCircle size={14} className="shrink-0" />
            <span>配置保存失败：{saveError}</span>
          </div>
        )}

        <div className="text-center mt-4">
          <button
            onClick={handleFinish}
            disabled={finishing || certInstalling}
            className="text-xs themed-text-muted hover:themed-text-secondary transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
          >
            {finishing ? "正在配置..." : "跳过引导，使用默认设置"}
          </button>
        </div>
      </div>
    </div>
  );
}
