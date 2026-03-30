import type { AppConfig } from "./stores/appStore";
import type { SettingsForm } from "./types";

export type { SettingsForm };

export const DEFAULT_WHISTLE_HOST = "127.0.0.1";
export const DEFAULT_WHISTLE_PORT = 18899;
export const DEFAULT_EXTERNAL_HOST = "127.0.0.1";
export const DEFAULT_EXTERNAL_PORT = 8899;
export const DEFAULT_PAC_PORT = 18901;
export const DEFAULT_AUTH_PROXY_PORT = 18900;
export const DEFAULT_PROXY_BYPASS = "localhost;127.*;10.*;172.16.*;192.168.*;<local>";

export const DEFAULT_SETTINGS_FORM = {
  whistleMode: "embedded",
  whistleHost: DEFAULT_WHISTLE_HOST,
  whistlePort: DEFAULT_WHISTLE_PORT,
  username: "",
  password: "",
  externalHost: DEFAULT_EXTERNAL_HOST,
  externalPort: DEFAULT_EXTERNAL_PORT,
  externalUsername: "",
  externalPassword: "",
  socksPort: 0,
  timeout: 60,
  storagePath: "",
  upstreamProxy: "",
  interceptHttps: true,
  autoStartWhistle: true,
  autoStartProxy: false,
  pacServerPort: DEFAULT_PAC_PORT,
  authProxyPort: DEFAULT_AUTH_PROXY_PORT,
  proxyBypass: DEFAULT_PROXY_BYPASS,
  localAuthBypass: false,
  trayClickAction: "show_window",
  minimizeToTray: true,
} as const;

export function buildFallbackConfig(): AppConfig {
  return {
    whistle: {
      mode: "embedded",
      host: DEFAULT_WHISTLE_HOST,
      port: DEFAULT_WHISTLE_PORT,
      username: "",
      password: "",
      socks_port: 0,
      timeout: 60,
      storage_path: "",
      upstream_proxy: "",
      intercept_https: true,
    },
    proxy_mode: "direct",
    active_profile_id: "default",
    profiles: [{ id: "default", name: "默认配置", rules: [] }],
    auto_start_whistle: true,
    auto_start_proxy: false,
    pac_server_port: DEFAULT_PAC_PORT,
    auth_proxy_port: DEFAULT_AUTH_PROXY_PORT,
    app_settings: {
      minimize_to_tray: true,
      start_on_boot: false,
      theme: "dark",
      proxy_bypass: DEFAULT_PROXY_BYPASS,
      local_auth_bypass: false,
      tray_click_action: "show_window",
      last_proxy_mode: "direct",
      external_host: DEFAULT_EXTERNAL_HOST,
      external_port: DEFAULT_EXTERNAL_PORT,
      external_username: "",
      external_password: "",
    },
    setup_completed: true,
  };
}
