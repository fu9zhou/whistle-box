export const VALID_PAGES = [
  "dashboard",
  "proxy",
  "rules",
  "whistle",
  "config",
  "settings",
] as const;
export type Page = (typeof VALID_PAGES)[number];

export type SettingsForm = {
  whistleMode: "embedded" | "external";
  whistleHost: string;
  whistlePort: number;
  username: string;
  password: string;
  externalHost: string;
  externalPort: number;
  externalUsername: string;
  externalPassword: string;
  socksPort: number;
  timeout: number;
  storagePath: string;
  upstreamProxy: string;
  interceptHttps: boolean;
  autoStartWhistle: boolean;
  autoStartProxy: boolean;
  pacServerPort: number;
  authProxyPort: number;
  proxyBypass: string;
  localAuthBypass: boolean;
  trayClickAction: "show_window" | "toggle_direct_rule" | "toggle_direct_global" | "cycle";
  minimizeToTray: boolean;
};
