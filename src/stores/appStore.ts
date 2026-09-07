import { create } from "zustand";
import { invoke } from "@tauri-apps/api/core";

export interface WhistleConnection {
  mode: string;
  host: string;
  port: number;
  username: string;
  password: string;
  socks_port: number;
  timeout: number;
  storage_path: string;
  upstream_proxy: string;
  intercept_https: boolean;
}

export interface AppSettings {
  minimize_to_tray: boolean;
  start_on_boot: boolean;
  theme: string;
  proxy_bypass: string;
  local_auth_bypass: boolean;
  tray_click_action: string;
  last_proxy_mode: string;
  external_host?: string;
  external_port?: number;
  external_username?: string;
  external_password?: string;
}

export interface ProxyRule {
  id: string;
  pattern: string;
  enabled: boolean;
  comment: string;
}

export interface Profile {
  id: string;
  name: string;
  rules: ProxyRule[];
}

export interface AppConfig {
  whistle: WhistleConnection;
  proxy_mode: string;
  active_profile_id: string;
  profiles: Profile[];
  auto_start_whistle: boolean;
  auto_start_proxy: boolean;
  pac_server_port: number;
  auth_proxy_port: number;
  app_settings: AppSettings;
  setup_completed?: boolean;
}

export interface WhistleStatus {
  running: boolean;
  mode: string;
  host: string;
  port: number;
  pid: number;
  uptime_check: boolean;
}

export interface ProxyStatus {
  enabled: boolean;
  mode: string;
  host: string;
  port: number;
  pac_url: string | null;
}

interface AppState {
  config: AppConfig | null;
  whistleStatus: WhistleStatus | null;
  proxyStatus: ProxyStatus | null;
  authProxyUrl: string | null;
  loading: boolean;
  error: string | null;
  theme: "dark" | "light";

  loadConfig: () => Promise<void>;
  saveConfig: (config: AppConfig) => Promise<void>;

  startWhistle: () => Promise<void>;
  stopWhistle: () => Promise<void>;
  refreshWhistleStatus: () => Promise<void>;

  setProxyMode: (mode: string) => Promise<void>;
  refreshProxyStatus: () => Promise<void>;

  startAuthProxy: () => Promise<void>;
  startPacServer: () => Promise<void>;
  refreshPac: () => Promise<void>;

  exportConfig: (path: string) => Promise<void>;
  importConfig: (path: string) => Promise<void>;

  exportWhistleRules: (path: string) => Promise<void>;
  importWhistleRules: (path: string) => Promise<void>;

  switchProfile: (profileId: string) => Promise<void>;

  checkExternalWhistle: (
    host: string,
    port: number,
    username: string,
    password: string,
  ) => Promise<boolean>;

  needsWhistleRestart: boolean;
  needsAppRestart: boolean;
  setNeedsWhistleRestart: (v: boolean) => void;
  setNeedsAppRestart: (v: boolean) => void;

  toggleTheme: () => void;
  clearError: () => void;
}

const getInitialTheme = (): "dark" | "light" => {
  try {
    const saved = localStorage.getItem("whistlebox-theme");
    if (saved === "light" || saved === "dark") return saved;
  } catch {}
  if (window.matchMedia?.("(prefers-color-scheme: light)").matches) return "light";
  return "dark";
};

export const useAppStore = create<AppState>((set, get) => {
  let whistleRefresh: Promise<void> | null = null;
  let proxyRefresh: Promise<void> | null = null;
  const setError = (e: unknown) => set({ error: String(e) });
  const withLoading = async <T>(fn: () => Promise<T>): Promise<T> => {
    try {
      set({ loading: true, error: null });
      const result = await fn();
      set({ loading: false });
      return result;
    } catch (e) {
      set({ error: String(e), loading: false });
      throw e;
    }
  };
  const quietly = async (fn: () => Promise<void>) => {
    try {
      await fn();
    } catch (e) {
      setError(e);
    }
  };
  const perform = async (fn: () => Promise<void>) => {
    try {
      await fn();
    } catch (e) {
      setError(e);
      throw e;
    }
  };

  return {
    config: null,
    whistleStatus: {
      running: false,
      mode: "embedded",
      host: "127.0.0.1",
      port: 0,
      pid: 0,
      uptime_check: false,
    },
    proxyStatus: null,
    authProxyUrl: null,
    loading: true,
    error: null,
    theme: getInitialTheme(),
    needsWhistleRestart: false,
    needsAppRestart: false,
    setNeedsWhistleRestart: (v) => set({ needsWhistleRestart: v }),
    setNeedsAppRestart: (v) => set({ needsAppRestart: v }),

    loadConfig: async () => {
      try {
        set({ loading: true, error: null });
        const config = await invoke<AppConfig>("cmd_get_config");
        set({ config, loading: false });
      } catch (e) {
        console.error("loadConfig failed:", e);
        set({ error: String(e), loading: false });
      }
    },

    saveConfig: async (config: AppConfig) => {
      try {
        const saved = await invoke<AppConfig>("cmd_save_config", {
          config,
          baseConfig: get().config,
        });
        set({ config: saved || config });
        const url = await invoke<string>("cmd_get_auth_proxy_url").catch(() => null);
        if (url) set({ authProxyUrl: url });
      } catch (e) {
        console.error("saveConfig failed:", e);
        setError(e);
        throw e;
      }
    },

    startWhistle: async () => {
      await withLoading(async () => {
        const status = await invoke<WhistleStatus>("cmd_start_whistle");
        set({ whistleStatus: status });
      });
    },

    stopWhistle: async () => {
      await perform(async () => {
        await invoke("cmd_stop_whistle");
        const prev = get().whistleStatus;
        set({
          whistleStatus: {
            running: false,
            mode: prev?.mode ?? "embedded",
            host: prev?.host ?? "127.0.0.1",
            port: prev?.port ?? 18899,
            pid: 0,
            uptime_check: false,
          },
        });
      });
    },

    refreshWhistleStatus: () => {
      whistleRefresh ??= quietly(async () => {
        const status = await invoke<WhistleStatus>("cmd_get_whistle_status");
        set({ whistleStatus: status });
      }).finally(() => {
        whistleRefresh = null;
      });
      return whistleRefresh;
    },

    setProxyMode: async (mode: string) => {
      await withLoading(async () => {
        await invoke("cmd_set_proxy_mode", { mode });
        const proxyStatus = await invoke<ProxyStatus>("cmd_get_proxy_status");
        const config = await invoke<AppConfig>("cmd_get_config");
        set({ proxyStatus, config });
      });
    },

    refreshProxyStatus: () => {
      proxyRefresh ??= quietly(async () => {
        const proxyStatus = await invoke<ProxyStatus>("cmd_get_proxy_status");
        set({ proxyStatus });
      }).finally(() => {
        proxyRefresh = null;
      });
      return proxyRefresh;
    },

    startAuthProxy: async () => {
      await perform(async () => {
        const url = await invoke<string>("cmd_start_auth_proxy");
        set({ authProxyUrl: url });
      });
    },

    startPacServer: async () => {
      await perform(async () => {
        await invoke<number>("cmd_start_pac_server");
      });
    },

    refreshPac: async () => {
      try {
        await invoke("cmd_refresh_pac");
      } catch (e) {
        set({ error: `刷新 PAC 规则失败: ${e}` });
        throw e;
      }
    },

    exportConfig: async (path: string) => {
      await perform(async () => {
        await invoke("cmd_export_config", { path });
      });
    },

    importConfig: async (path: string) => {
      await perform(async () => {
        const config = await invoke<AppConfig>("cmd_import_config", { path });
        set({ config });
        await get().refreshPac();
      });
    },

    exportWhistleRules: async (path: string) => {
      await perform(async () => {
        await invoke("cmd_export_whistle_rules", { path });
      });
    },

    importWhistleRules: async (path: string) => {
      await perform(async () => {
        await invoke("cmd_import_whistle_rules", { path });
      });
    },

    switchProfile: async (profileId: string) => {
      await perform(async () => {
        await invoke("cmd_switch_profile", { profileId });
        await get().loadConfig();
        await get().refreshPac();
      });
    },

    checkExternalWhistle: async (
      host: string,
      port: number,
      username: string,
      password: string,
    ) => {
      try {
        return await invoke<boolean>("cmd_check_external_whistle", {
          host,
          port,
          username,
          password,
        });
      } catch {
        return false;
      }
    },

    toggleTheme: () => {
      const current = get().theme;
      const next = current === "dark" ? "light" : "dark";
      try {
        localStorage.setItem("whistlebox-theme", next);
      } catch {}
      set({ theme: next });
    },

    clearError: () => set({ error: null }),
  };
});
