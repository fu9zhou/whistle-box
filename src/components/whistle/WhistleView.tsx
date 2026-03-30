import { useEffect, useState, useRef, useCallback } from "react";
import {
  ExternalLink,
  RefreshCw,
  RotateCcw,
  Play,
  Square,
  AlertTriangle,
  Loader2,
  Unplug,
} from "lucide-react";
import { invoke } from "@tauri-apps/api/core";
import { useAppStore } from "../../stores/appStore";

function isPrivateIPv4(hostname: string): boolean {
  const parts = hostname.split(".");
  if (parts.length !== 4) return false;
  const nums = parts.map((p) => Number(p));
  if (nums.some((n) => !Number.isInteger(n) || n < 0 || n > 255)) return false;
  return (
    nums[0] === 10 ||
    (nums[0] === 172 && nums[1] >= 16 && nums[1] <= 31) ||
    (nums[0] === 192 && nums[1] === 168)
  );
}

function isAllowedHost(hostname: string): boolean {
  const lower = hostname.toLowerCase();
  return lower === "localhost" || lower === "127.0.0.1" || lower === "::1" || isPrivateIPv4(lower);
}

function buildSafeHttpUrl(host: string, port: number): string | null {
  if (!Number.isInteger(port) || port < 1 || port > 65535) return null;
  try {
    const url = new URL(`http://${host}:${port}`);
    if ((url.protocol === "http:" || url.protocol === "https:") && isAllowedHost(url.hostname)) {
      return url.toString();
    }
  } catch {
    // ignore invalid URL
  }
  return null;
}

function isSafeAuthProxyUrl(rawUrl: string): boolean {
  try {
    const url = new URL(rawUrl);
    const port = Number(url.port || "80");
    return url.protocol === "http:" && port >= 1 && port <= 65535 && isAllowedHost(url.hostname);
  } catch {
    return false;
  }
}

async function probeAuthProxy(): Promise<boolean> {
  try {
    return await invoke<boolean>("cmd_probe_auth_proxy");
  } catch {
    return false;
  }
}

export default function WhistleView() {
  const whistleStatus = useAppStore((s) => s.whistleStatus);
  const authProxyUrl = useAppStore((s) => s.authProxyUrl);
  const config = useAppStore((s) => s.config);
  const startWhistle = useAppStore((s) => s.startWhistle);
  const stopWhistle = useAppStore((s) => s.stopWhistle);
  const startAuthProxy = useAppStore((s) => s.startAuthProxy);
  const refreshWhistleStatus = useAppStore((s) => s.refreshWhistleStatus);
  const theme = useAppStore((s) => s.theme);

  const [iframeKey, setIframeKey] = useState(0);
  const [loading, setLoading] = useState(false);
  const [disconnectAlert, setDisconnectAlert] = useState(false);
  const [connecting, setConnecting] = useState(false);
  const [authProxyReady, setAuthProxyReady] = useState(false);
  const [probeFailed, setProbeFailed] = useState(false);
  const prevAliveRef = useRef<boolean | null>(null);
  const iframeRetryTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  const whistleAlive = whistleStatus?.running && whistleStatus?.uptime_check;
  const isEmbedded = config?.whistle?.mode === "embedded";
  const configLoaded = config !== null;

  useEffect(() => {
    if (!whistleAlive || !authProxyUrl) {
      setAuthProxyReady(false);
      setProbeFailed(false);
      return;
    }
    let cancelled = false;
    const check = async () => {
      setProbeFailed(false);
      for (let i = 0; i < 30 && !cancelled; i++) {
        if (await probeAuthProxy()) {
          if (!cancelled) {
            setAuthProxyReady(true);
            setProbeFailed(false);
          }
          return;
        }
        const delay = Math.min(500 + i * 300, 3000);
        await new Promise((r) => setTimeout(r, delay));
      }
      if (!cancelled) setProbeFailed(true);
    };
    check();
    return () => { cancelled = true; };
  }, [whistleAlive, authProxyUrl, iframeKey]);

  useEffect(() => {
    if (!authProxyReady || !whistleAlive) return;
    iframeRetryTimerRef.current = setTimeout(async () => {
      const ok = await probeAuthProxy();
      if (ok) {
        setIframeKey((k) => k + 1);
      }
    }, 6000);
    return () => {
      if (iframeRetryTimerRef.current) clearTimeout(iframeRetryTimerRef.current);
    };
  }, [authProxyReady, iframeKey, whistleAlive]);

  const handleConnectExternal = useCallback(async () => {
    setConnecting(true);
    setAuthProxyReady(false);
    try {
      await startAuthProxy();
      await refreshWhistleStatus();
      setIframeKey((k) => k + 1);
    } finally {
      setConnecting(false);
    }
  }, [startAuthProxy, refreshWhistleStatus]);

  const prevAuthProxyUrlRef = useRef(authProxyUrl);

  useEffect(() => {
    refreshWhistleStatus().catch(() => { });
    startAuthProxy()
      .then(() => setIframeKey((k) => k + 1))
      .catch(() => { });
  }, []); // eslint-disable-line react-hooks/exhaustive-deps

  useEffect(() => {
    if (
      authProxyUrl &&
      prevAuthProxyUrlRef.current &&
      authProxyUrl !== prevAuthProxyUrlRef.current
    ) {
      setAuthProxyReady(false);
      setIframeKey((k) => k + 1);
    }
    prevAuthProxyUrlRef.current = authProxyUrl;
  }, [authProxyUrl]);

  const prevAliveForRefreshRef = useRef<boolean | null>(null);
  useEffect(() => {
    const wasAlive = prevAliveForRefreshRef.current;
    const nowAlive = whistleAlive ?? false;
    if (isEmbedded && nowAlive && (wasAlive === false || wasAlive === null)) {
      setAuthProxyReady(false);
      startAuthProxy()
        .then(() => setIframeKey((k) => k + 1))
        .catch(() => { });
    }
    prevAliveForRefreshRef.current = nowAlive;
  }, [whistleAlive, isEmbedded, startAuthProxy]);

  useEffect(() => {
    if (configLoaded && !isEmbedded) {
      handleConnectExternal();
      const interval = setInterval(() => {
        refreshWhistleStatus();
      }, 15000);
      return () => clearInterval(interval);
    }
  }, [isEmbedded, configLoaded, handleConnectExternal, refreshWhistleStatus]);

  useEffect(() => {
    if (!isEmbedded && prevAliveRef.current === true && !whistleAlive) {
      setDisconnectAlert(true);
    }
    prevAliveRef.current = whistleAlive ?? null;
  }, [whistleAlive, isEmbedded]);

  const handleStart = async () => {
    setLoading(true);
    setAuthProxyReady(false);
    try {
      await startWhistle();
      await startAuthProxy();
      await new Promise((r) => setTimeout(r, 1500));
      setIframeKey((k) => k + 1);
    } finally {
      setLoading(false);
    }
  };

  const handleStop = async () => {
    setAuthProxyReady(false);
    await stopWhistle();
  };

  const handleRestart = async () => {
    setLoading(true);
    setAuthProxyReady(false);
    try {
      await stopWhistle();
      await new Promise((r) => setTimeout(r, 1500));
      await startWhistle();
      await startAuthProxy();
      await new Promise((r) => setTimeout(r, 1500));
      setIframeKey((k) => k + 1);
    } finally {
      setLoading(false);
    }
  };

  const handleRefresh = () => {
    setAuthProxyReady(false);
    setIframeKey((k) => k + 1);
  };

  const extHost = config?.app_settings?.external_host ?? "127.0.0.1";
  const extPort = config?.app_settings?.external_port ?? 8899;
  const embHost = config?.whistle?.host ?? "127.0.0.1";
  const embPort = config?.whistle?.port ?? 18899;

  const whistleDirectUrl = isEmbedded
    ? buildSafeHttpUrl(embHost, embPort)
    : buildSafeHttpUrl(extHost, extPort);
  const localBypass = config?.app_settings?.local_auth_bypass;
  const authProxyPort = config?.auth_proxy_port ?? 18900;
  const bypassUrl = buildSafeHttpUrl("127.0.0.1", authProxyPort);
  const externalUrl = localBypass ? bypassUrl : whistleDirectUrl;

  const handleOpenExternal = async () => {
    if (!externalUrl) return;
    try {
      const url = new URL(externalUrl);
      if ((url.protocol !== "http:" && url.protocol !== "https:") || !isAllowedHost(url.hostname)) {
        return;
      }
      const { open } = await import("@tauri-apps/plugin-shell");
      await open(externalUrl);
    } catch {
      // Keep failure local to avoid bypassing shell allowlist via window.open fallback.
    }
  };

  const renderToolbarButtons = () => {
    if (isEmbedded) {
      if (whistleAlive) {
        return (
          <>
            <button
              onClick={handleOpenExternal}
              className="btn-secondary text-xs flex items-center gap-1.5 py-1.5"
            >
              <ExternalLink size={12} /> 浏览器打开
            </button>
            <button
              onClick={handleRefresh}
              className="btn-secondary text-xs flex items-center gap-1.5 py-1.5"
            >
              <RefreshCw size={12} /> 刷新
            </button>
            <button
              onClick={handleRestart}
              disabled={loading}
              className="btn-secondary text-xs flex items-center gap-1.5 py-1.5"
            >
              {loading ? <Loader2 size={12} className="animate-spin" /> : <RotateCcw size={12} />}{" "}
              重启
            </button>
            <button
              onClick={handleStop}
              className="btn-danger text-xs flex items-center gap-1.5 py-1.5"
            >
              <Square size={12} /> 停止
            </button>
          </>
        );
      }
      return (
        <button
          onClick={handleStart}
          disabled={loading}
          className="btn-primary text-xs flex items-center gap-1.5 py-1.5"
        >
          {loading ? <Loader2 size={12} className="animate-spin" /> : <Play size={12} />}
          {loading ? "启动中..." : "启动 Whistle"}
        </button>
      );
    }

    // External mode
    return (
      <>
        {whistleAlive && (
          <>
            <button
              onClick={handleOpenExternal}
              className="btn-secondary text-xs flex items-center gap-1.5 py-1.5"
            >
              <ExternalLink size={12} /> 浏览器打开
            </button>
            <button
              onClick={handleRefresh}
              className="btn-secondary text-xs flex items-center gap-1.5 py-1.5"
            >
              <RefreshCw size={12} /> 刷新
            </button>
          </>
        )}
        <button
          onClick={handleConnectExternal}
          disabled={connecting}
          className="btn-secondary text-xs flex items-center gap-1.5 py-1.5"
        >
          {connecting ? <Loader2 size={12} className="animate-spin" /> : <RefreshCw size={12} />}
          {connecting ? "连接中..." : "重新连接"}
        </button>
      </>
    );
  };

  const iframeSrc = (() => {
    if (!authProxyUrl || !isSafeAuthProxyUrl(authProxyUrl)) return "";
    const separator = authProxyUrl.includes("?") ? "&" : "?";
    return `${authProxyUrl}${separator}_theme=${theme}`;
  })();

  const renderContent = () => {
    if (whistleAlive && authProxyUrl && authProxyReady) {
      return (
        <iframe
          key={`${iframeKey}-${theme}`}
          src={iframeSrc}
          className="w-full h-full border-0"
          title="Whistle UI"
        />
      );
    }

    if (whistleAlive && authProxyUrl && !authProxyReady) {
      return (
        <div className="absolute inset-0 flex items-center justify-center themed-main">
          <div className="text-center space-y-4">
            {probeFailed ? (
              <>
                <AlertTriangle size={40} className="text-warning-400 mx-auto" />
                <div className="text-surface-400">无法连接到代理界面</div>
                <div className="text-xs text-surface-600">代理服务可能仍在启动中，请稍后重试</div>
                <button
                  onClick={() => {
                    setProbeFailed(false);
                    setIframeKey((k) => k + 1);
                  }}
                  className="btn-primary text-xs flex items-center gap-1.5 py-1.5 mx-auto mt-2"
                >
                  <RefreshCw size={12} /> 重试连接
                </button>
              </>
            ) : (
              <>
                <Loader2 size={40} className="animate-spin text-accent-500 mx-auto" />
                <div className="text-surface-400">正在连接 Whistle 界面...</div>
                <div className="text-xs text-surface-600">等待代理服务就绪</div>
              </>
            )}
          </div>
        </div>
      );
    }

    if (isEmbedded) {
      return (
        <div className="absolute inset-0 flex items-center justify-center themed-main">
          <div className="text-center space-y-4">
            {loading ? (
              <>
                <Loader2 size={40} className="animate-spin text-accent-500 mx-auto" />
                <div className="text-surface-400">正在启动 Whistle...</div>
                <div className="text-xs text-surface-600">首次启动可能需要几秒钟</div>
              </>
            ) : whistleStatus?.running && !whistleStatus?.uptime_check ? (
              <>
                <AlertTriangle size={40} className="text-warning-400 mx-auto" />
                <div className="text-surface-400">Whistle 进程存在但无响应</div>
                <div className="text-xs text-surface-600">正在尝试自动恢复...</div>
              </>
            ) : (
              <>
                <div className="w-16 h-16 rounded-2xl bg-surface-900 flex items-center justify-center mx-auto">
                  <Play size={28} className="text-surface-600 ml-1" />
                </div>
                <div className="text-surface-400">内置 Whistle 未启动</div>
                <div className="text-xs text-surface-600">点击上方"启动 Whistle"按钮开始</div>
              </>
            )}
          </div>
        </div>
      );
    }

    // External mode - not connected
    return (
      <div className="absolute inset-0 flex items-center justify-center themed-main">
        <div className="text-center space-y-4">
          {connecting ? (
            <>
              <Loader2 size={40} className="animate-spin text-accent-500 mx-auto" />
              <div className="text-surface-400">正在连接外部 Whistle...</div>
              <div className="text-xs text-surface-600">
                {extHost}:{extPort}
              </div>
            </>
          ) : (
            <>
              <div className="w-16 h-16 rounded-2xl bg-surface-900 flex items-center justify-center mx-auto">
                <Unplug size={28} className="text-surface-600" />
              </div>
              <div className="text-surface-400">无法连接外部 Whistle</div>
              <div className="text-xs text-surface-600">
                请确认 {extHost}:{extPort} 上的 Whistle 实例正在运行
              </div>
              <button
                onClick={handleConnectExternal}
                className="btn-primary text-xs flex items-center gap-1.5 py-1.5 mx-auto"
              >
                <RefreshCw size={12} /> 重试连接
              </button>
            </>
          )}
        </div>
      </div>
    );
  };

  return (
    <div className="flex flex-col h-full animate-fade-in">
      {/* Toolbar */}
      <div className="flex items-center justify-between px-5 py-3 themed-sidebar shrink-0">
        <div className="flex items-center gap-3">
          <h1 className="text-base font-semibold text-surface-200">Whistle</h1>
          <div className="flex items-center gap-1.5">
            <span
              className={`status-dot ${whistleAlive
                  ? "status-dot--active"
                  : whistleStatus?.running
                    ? "status-dot--warning"
                    : "status-dot--inactive"
                }`}
            />
            <span className="text-xs text-surface-500">
              {isEmbedded
                ? whistleAlive
                  ? "内置 Whistle 运行中"
                  : whistleStatus?.running
                    ? "内置 Whistle 异常"
                    : "内置 Whistle 未启动"
                : whistleAlive
                  ? `已连接 ${extHost}:${extPort}`
                  : `未连接 ${extHost}:${extPort}`}
            </span>
          </div>
        </div>
        <div className="flex items-center gap-2">{renderToolbarButtons()}</div>
      </div>

      {/* Content */}
      <div className="flex-1 relative bg-white">{renderContent()}</div>

      {/* Disconnect Alert - external mode only */}
      {disconnectAlert && (
        <div className="absolute inset-0 bg-black/50 flex items-center justify-center z-50">
          <div className="glass-panel p-6 max-w-sm mx-4 text-center space-y-4">
            <AlertTriangle size={36} className="text-warning-400 mx-auto" />
            <div className="text-surface-200 font-semibold">外部 Whistle 已断开</div>
            <div className="text-xs text-surface-400">
              {extHost}:{extPort} 上的 Whistle 实例已停止响应。
            </div>
            <div className="flex gap-3 justify-center">
              <button
                onClick={() => {
                  setDisconnectAlert(false);
                  handleConnectExternal();
                }}
                className="btn-secondary text-xs py-1.5 px-4"
              >
                重新连接
              </button>
              <button
                onClick={() => setDisconnectAlert(false)}
                className="btn-primary text-xs py-1.5 px-4"
              >
                知道了
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
