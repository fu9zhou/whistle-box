import { useEffect, useState, useRef, useCallback, useMemo } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useAppStore, type AppConfig } from "../../stores/appStore";
import { DEFAULT_SETTINGS_FORM, type SettingsForm } from "../../defaults";

export type SettingsFormState = SettingsForm;

const whistleRestartFields = [
  "whistleHost",
  "whistlePort",
  "username",
  "password",
  "socksPort",
  "timeout",
  "storagePath",
  "upstreamProxy",
] as const;
const appRestartFields = ["pacServerPort", "authProxyPort"] as const;

function buildConfig(f: SettingsFormState, base: AppConfig): AppConfig {
  return {
    ...base,
    whistle: {
      mode: f.whistleMode,
      host: f.whistleHost,
      port: f.whistlePort,
      username: f.username,
      password: f.password,
      socks_port: f.socksPort,
      timeout: f.timeout,
      storage_path: f.storagePath,
      upstream_proxy: f.upstreamProxy,
      intercept_https: f.interceptHttps,
    },
    auto_start_whistle: f.autoStartWhistle,
    auto_start_proxy: f.autoStartProxy,
    pac_server_port: f.pacServerPort,
    auth_proxy_port: f.authProxyPort,
    setup_completed: base.setup_completed ?? true,
    app_settings: {
      ...base.app_settings,
      proxy_bypass: f.proxyBypass,
      local_auth_bypass: f.localAuthBypass,
      tray_click_action: f.trayClickAction,
      minimize_to_tray: f.minimizeToTray,
      external_host: f.externalHost,
      external_port: f.externalPort,
      external_username: f.externalUsername,
      external_password: f.externalPassword,
    },
  };
}

export function useSettingsForm() {
  const config = useAppStore((s) => s.config);
  const saveConfig = useAppStore((s) => s.saveConfig);
  const checkExternalWhistle = useAppStore((s) => s.checkExternalWhistle);
  const whistleStatus = useAppStore((s) => s.whistleStatus);
  const startWhistle = useAppStore((s) => s.startWhistle);
  const stopWhistle = useAppStore((s) => s.stopWhistle);
  const startAuthProxy = useAppStore((s) => s.startAuthProxy);

  const embeddedRunning = whistleStatus?.running && whistleStatus?.mode === "embedded";

  const defaultForm = useMemo(() => ({ ...DEFAULT_SETTINGS_FORM }) as SettingsFormState, []);
  const [form, setForm] = useState<SettingsFormState>(defaultForm);
  const [savedForm, setSavedForm] = useState<SettingsFormState>(defaultForm);

  const needsWhistleRestart = useAppStore((s) => s.needsWhistleRestart);
  const setNeedsWhistleRestart = useAppStore((s) => s.setNeedsWhistleRestart);
  const needsAppRestart = useAppStore((s) => s.needsAppRestart);
  const setNeedsAppRestart = useAppStore((s) => s.setNeedsAppRestart);

  const [autoStartEnabled, setAutoStartEnabled] = useState(false);
  const [testing, setTesting] = useState(false);
  const [testResult, setTestResult] = useState<boolean | null>(null);
  const [showResetConfirm, setShowResetConfirm] = useState(false);
  const [whistleRestarting, setWhistleRestarting] = useState(false);
  const [whistleStarting, setWhistleStarting] = useState(false);
  const [saveToast, setSaveToast] = useState<string | null>(null);
  const [certInstalling, setCertInstalling] = useState(false);
  const [certRemoving, setCertRemoving] = useState(false);
  const [certResult, setCertResult] = useState<{ ok: boolean; msg: string } | null>(null);
  const [certInstalled, setCertInstalled] = useState<boolean | null>(null);
  const [showPassword, setShowPassword] = useState(false);

  const configLoadedRef = useRef(false);
  const configRef = useRef(config);
  configRef.current = config;
  const saveTimerRef = useRef<ReturnType<typeof setTimeout>>();
  const savePendingRef = useRef(false);
  const saveQueueRef = useRef<Promise<void>>(Promise.resolve());
  const pendingSaveCountRef = useRef(0);
  const saveRevisionRef = useRef(0);
  const toastTimerRef = useRef<ReturnType<typeof setTimeout>>();
  const testTimerRef = useRef<ReturnType<typeof setTimeout>>();

  const enqueueSave = useCallback((task: () => Promise<void>) => {
    pendingSaveCountRef.current += 1;
    savePendingRef.current = true;
    saveQueueRef.current = saveQueueRef.current
      .catch(() => {})
      .then(task)
      .finally(() => {
        pendingSaveCountRef.current = Math.max(0, pendingSaveCountRef.current - 1);
        if (pendingSaveCountRef.current === 0) {
          savePendingRef.current = false;
        }
      });
  }, []);

  const checkCertStatus = useCallback(() => {
    invoke<boolean>("cmd_check_cert_installed")
      .then(setCertInstalled)
      .catch(() => setCertInstalled(null));
  }, []);

  useEffect(() => {
    invoke<boolean>("cmd_get_autostart")
      .then(setAutoStartEnabled)
      .catch(() => {});
    checkCertStatus();
    return () => {
      if (saveTimerRef.current) clearTimeout(saveTimerRef.current);
      if (toastTimerRef.current) clearTimeout(toastTimerRef.current);
      if (testTimerRef.current) clearTimeout(testTimerRef.current);
    };
  }, []);

  useEffect(() => {
    if (config && !savePendingRef.current) {
      const loaded: SettingsFormState = {
        whistleMode: config.whistle.mode as "embedded" | "external",
        whistleHost: config.whistle.host,
        whistlePort: config.whistle.port,
        username: config.whistle.username,
        password: config.whistle.password,
        externalHost: config.app_settings?.external_host || "127.0.0.1",
        externalPort: config.app_settings?.external_port || 8899,
        externalUsername: config.app_settings?.external_username || "",
        externalPassword: config.app_settings?.external_password || "",
        socksPort: config.whistle.socks_port || 0,
        timeout: config.whistle.timeout || 60,
        storagePath: config.whistle.storage_path || "",
        upstreamProxy: config.whistle.upstream_proxy || "",
        interceptHttps: config.whistle.intercept_https || false,
        autoStartWhistle: config.auto_start_whistle,
        autoStartProxy: config.auto_start_proxy,
        pacServerPort: config.pac_server_port,
        authProxyPort: config.auth_proxy_port,
        proxyBypass:
          config.app_settings?.proxy_bypass || "localhost;127.*;10.*;172.16.*;192.168.*;<local>",
        localAuthBypass: config.app_settings?.local_auth_bypass ?? false,
        trayClickAction: (config.app_settings?.tray_click_action ||
          "show_window") as SettingsForm["trayClickAction"],
        minimizeToTray: config.app_settings?.minimize_to_tray !== false,
      };
      setForm(loaded);
      setSavedForm(loaded);
      configLoadedRef.current = true;
    }
  }, [config]);

  const isDefault = (() => {
    const ignore = new Set(["username", "password", "externalUsername", "externalPassword"]);
    return Object.keys(defaultForm).every(
      (k) =>
        ignore.has(k) ||
        JSON.stringify(savedForm[k as keyof typeof savedForm]) ===
          JSON.stringify(defaultForm[k as keyof typeof defaultForm]),
    );
  })();

  const doSave = useCallback(
    async (
      newForm: SettingsFormState,
      toastMsg: string,
      needsProxyUpdate: boolean,
      changedHttpsIntercept: boolean,
      changedWhistle: boolean,
      changedApp: boolean,
      revision: number,
    ) => {
      if (revision !== saveRevisionRef.current) return;
      const latestConfig = configRef.current;
      if (!latestConfig) return;
      const newConfig = buildConfig(newForm, latestConfig);
      await saveConfig(newConfig);
      if (revision !== saveRevisionRef.current) return;
      setSavedForm(newForm);
      if (changedWhistle) setNeedsWhistleRestart(true);
      if (changedApp) setNeedsAppRestart(true);
      if (needsProxyUpdate) {
        await startAuthProxy();
      }
      if (changedHttpsIntercept) {
        await invoke("cmd_sync_https_interception", { enable: newForm.interceptHttps }).catch(
          () => {},
        );
      }
      if (toastTimerRef.current) clearTimeout(toastTimerRef.current);
      setSaveToast(toastMsg);
      toastTimerRef.current = setTimeout(() => setSaveToast(null), 3000);
    },
    [saveConfig, startAuthProxy, setNeedsWhistleRestart, setNeedsAppRestart],
  );

  const autoSave = useCallback(
    (newForm: SettingsFormState, immediate: boolean = false) => {
      if (!config || !configLoadedRef.current) return;

      const whistleFieldMap: Record<string, unknown> = {
        whistleHost: config.whistle.host,
        whistlePort: config.whistle.port,
        username: config.whistle.username,
        password: config.whistle.password,
        socksPort: config.whistle.socks_port || 0,
        timeout: config.whistle.timeout || 60,
        storagePath: config.whistle.storage_path || "",
        upstreamProxy: config.whistle.upstream_proxy || "",
      };
      const changedWhistle =
        newForm.whistleMode === "embedded" &&
        whistleRestartFields.some((k) => newForm[k] !== whistleFieldMap[k]);

      const appFieldMap: Record<string, unknown> = {
        pacServerPort: config.pac_server_port,
        authProxyPort: config.auth_proxy_port,
      };
      const changedApp = appRestartFields.some((k) => newForm[k] !== appFieldMap[k]);

      const changedAuthBypass =
        newForm.localAuthBypass !== (config.app_settings?.local_auth_bypass ?? false);
      const changedHttpsIntercept =
        newForm.interceptHttps !== (config.whistle.intercept_https || false);
      const changedProxyTarget =
        newForm.whistleHost !== config.whistle.host ||
        newForm.whistlePort !== config.whistle.port ||
        newForm.username !== config.whistle.username ||
        newForm.password !== config.whistle.password ||
        newForm.externalHost !== (config.app_settings?.external_host || "127.0.0.1") ||
        newForm.externalPort !== (config.app_settings?.external_port || 8899);
      const needsProxyUpdate = changedAuthBypass || changedProxyTarget;

      const toastMsg =
        changedWhistle && embeddedRunning
          ? "设置已保存，需重启 Whistle 后生效"
          : changedApp
            ? "设置已保存，需重启应用后生效"
            : "设置已保存";

      if (saveTimerRef.current) clearTimeout(saveTimerRef.current);
      const revision = ++saveRevisionRef.current;

      if (immediate) {
        enqueueSave(() =>
          doSave(
            newForm,
            toastMsg,
            needsProxyUpdate,
            changedHttpsIntercept,
            changedWhistle,
            changedApp,
            revision,
          ),
        );
      } else {
        saveTimerRef.current = setTimeout(() => {
          enqueueSave(() =>
            doSave(
              newForm,
              toastMsg,
              needsProxyUpdate,
              changedHttpsIntercept,
              changedWhistle,
              changedApp,
              revision,
            ),
          );
        }, 800);
      }
    },
    [config, doSave, embeddedRunning, enqueueSave],
  );

  const updateForm = useCallback(
    (updates: Partial<SettingsFormState>, immediate?: boolean) => {
      setForm((prev) => {
        const newForm = { ...prev, ...updates };
        autoSave(newForm, immediate);
        return newForm;
      });
    },
    [autoSave],
  );

  useEffect(() => {
    if (form.whistleMode === "external" && configLoadedRef.current) {
      if (testTimerRef.current) clearTimeout(testTimerRef.current);
      testTimerRef.current = setTimeout(() => handleTestConnection(), 800);
    }
    return () => {
      if (testTimerRef.current) clearTimeout(testTimerRef.current);
    };
  }, [
    form.whistleMode,
    form.externalHost,
    form.externalPort,
    form.externalUsername,
    form.externalPassword,
  ]);

  const handleTestConnection = async () => {
    setTesting(true);
    setTestResult(null);
    const result = await checkExternalWhistle(
      form.externalHost,
      form.externalPort,
      form.externalUsername,
      form.externalPassword,
    );
    setTestResult(result);
    setTesting(false);
  };

  const handleRestartWhistle = async () => {
    setWhistleRestarting(true);
    try {
      if (saveTimerRef.current) clearTimeout(saveTimerRef.current);
      await stopWhistle();
      await new Promise((r) => setTimeout(r, 2000));
      if (!config) return;
      const latestConfig = buildConfig(form, config);
      await saveConfig(latestConfig);
      await new Promise((r) => setTimeout(r, 300));
      await startWhistle();
      await startAuthProxy();
      setNeedsWhistleRestart(false);
    } finally {
      setWhistleRestarting(false);
    }
  };

  const handleStartWhistle = async () => {
    if (!config) return;
    setWhistleStarting(true);
    try {
      if (saveTimerRef.current) clearTimeout(saveTimerRef.current);
      const latestConfig = buildConfig(form, config);
      await saveConfig(latestConfig);
      await startAuthProxy();
      await startWhistle();
      setNeedsWhistleRestart(false);
    } finally {
      setWhistleStarting(false);
    }
  };

  const handleSwitchToEmbedded = async () => {
    const newForm = { ...form, whistleMode: "embedded" as const };
    setForm(newForm);
    setNeedsWhistleRestart(false);
    if (saveTimerRef.current) clearTimeout(saveTimerRef.current);
    if (!config) return;
    const newConfig = buildConfig(newForm, config);
    await saveConfig(newConfig);
    await startAuthProxy();
    if (form.autoStartWhistle) {
      await startWhistle();
    }
  };

  const handleSwitchToExternal = async () => {
    if (embeddedRunning) {
      await stopWhistle();
    }
    const newForm = { ...form, whistleMode: "external" as const };
    setForm(newForm);
    setNeedsWhistleRestart(false);
    if (!config) return;
    if (saveTimerRef.current) clearTimeout(saveTimerRef.current);
    const newConfig = buildConfig(newForm, config);
    await saveConfig(newConfig);
    await startAuthProxy();
  };

  const handleResetForm = () => {
    const resetForm = {
      ...defaultForm,
      username: form.username,
      password: form.password,
      externalUsername: form.externalUsername,
      externalPassword: form.externalPassword,
    };
    updateForm(resetForm);
    setShowResetConfirm(false);
  };

  const handleInstallCert = async () => {
    setCertInstalling(true);
    setCertResult(null);
    try {
      const msg = await invoke<string>("cmd_install_cert");
      setCertResult({ ok: true, msg });
      setCertInstalled(true);
    } catch (e) {
      setCertResult({ ok: false, msg: String(e) });
    }
    setCertInstalling(false);
  };

  const handleUninstallCert = async () => {
    setCertRemoving(true);
    setCertResult(null);
    try {
      const msg = await invoke<string>("cmd_uninstall_cert");
      setCertResult({ ok: true, msg });
      setCertInstalled(false);
    } catch (e) {
      setCertResult({ ok: false, msg: String(e) });
    }
    setCertRemoving(false);
  };

  const handleToggleAutoStart = async (v: boolean) => {
    try {
      await invoke("cmd_set_autostart", { enabled: v });
      setAutoStartEnabled(v);
    } catch (e) {
      console.error("Failed to set autostart:", e);
    }
  };

  return {
    form,
    updateForm,
    config,
    embeddedRunning,
    isDefault,

    autoStartEnabled,
    testing,
    testResult,
    needsWhistleRestart,
    needsAppRestart,
    whistleRestarting,
    whistleStarting,
    showResetConfirm,
    setShowResetConfirm,
    saveToast,
    certInstalling,
    certRemoving,
    certResult,
    certInstalled,
    showPassword,
    setShowPassword,

    handleTestConnection,
    handleRestartWhistle,
    handleStartWhistle,
    handleSwitchToEmbedded,
    handleSwitchToExternal,
    handleResetForm,
    handleInstallCert,
    handleUninstallCert,
    handleToggleAutoStart,
  };
}
