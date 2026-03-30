import { useState, useEffect, useCallback } from "react";
import { Minus, Square, Copy, X, Sun, Moon } from "lucide-react";
import { useAppStore } from "../../stores/appStore";

export default function TitleBar() {
  const [isMaximized, setIsMaximized] = useState(false);
  const [appVersion, setAppVersion] = useState("v0.1.0");
  const theme = useAppStore((s) => s.theme);
  const toggleTheme = useAppStore((s) => s.toggleTheme);

  const syncMaximizedState = useCallback(async () => {
    try {
      const { getCurrentWindow } = await import("@tauri-apps/api/window");
      const maximized = await getCurrentWindow().isMaximized();
      setIsMaximized(maximized);
    } catch { }
  }, []);

  useEffect(() => {
    syncMaximizedState();
    let unlisten: (() => void) | undefined;
    let mounted = true;
    import("@tauri-apps/api/window")
      .then(({ getCurrentWindow }) => {
        getCurrentWindow()
          .onResized(() => {
            syncMaximizedState();
          })
          .then((fn) => {
            if (mounted) unlisten = fn;
            else fn();
          });
      })
      .catch(() => { });
    return () => {
      mounted = false;
      unlisten?.();
    };
  }, [syncMaximizedState]);

  useEffect(() => {
    import("@tauri-apps/api/app")
      .then(({ getVersion }) => getVersion().then((v) => setAppVersion(`v${v}`)))
      .catch(() => { });
  }, []);

  const handleMinimize = async () => {
    const { getCurrentWindow } = await import("@tauri-apps/api/window");
    getCurrentWindow().minimize();
  };

  const handleMaximize = async () => {
    const { getCurrentWindow } = await import("@tauri-apps/api/window");
    const win = getCurrentWindow();
    if (isMaximized) {
      await win.unmaximize();
    } else {
      await win.maximize();
    }
  };

  const handleClose = async () => {
    const { getCurrentWindow } = await import("@tauri-apps/api/window");
    getCurrentWindow().close();
  };

  return (
    <header className="drag-region h-10 flex items-center justify-between themed-titlebar select-none shrink-0">
      <div className="flex items-center gap-2.5 pl-4">
        <div className="flex items-center gap-1.5">
          <div className="w-3 h-3 rounded-full bg-gradient-to-br from-accent-400 to-accent-600 shadow-sm shadow-accent-500/30" />
          <div className="w-3 h-3 rounded-full bg-gradient-to-br from-accent-500/50 to-accent-700/50" />
        </div>
        <span className="text-sm font-semibold themed-text tracking-wide">WhistleBox</span>
        <span className="text-[10px] themed-text-muted font-mono">{appVersion}</span>
      </div>

      <div className="no-drag flex items-center h-full">
        <button
          onClick={toggleTheme}
          className="h-full px-3 flex items-center justify-center themed-text-secondary hover:themed-text transition-colors"
          title={theme === "dark" ? "切换亮色主题" : "切换暗色主题"}
        >
          {theme === "dark" ? <Sun size={14} /> : <Moon size={14} />}
        </button>
        <button
          onClick={handleMinimize}
          className="h-full px-3.5 flex items-center justify-center themed-text-secondary hover:themed-text transition-colors"
          aria-label="最小化"
        >
          <Minus size={14} />
        </button>
        <button
          onClick={handleMaximize}
          className="h-full px-3.5 flex items-center justify-center themed-text-secondary hover:themed-text transition-colors"
          aria-label={isMaximized ? "还原窗口" : "最大化"}
        >
          {isMaximized ? <Copy size={11} /> : <Square size={11} />}
        </button>
        <button
          onClick={handleClose}
          className="h-full px-3.5 flex items-center justify-center themed-text-secondary hover:text-white hover:bg-danger-600 transition-colors"
          aria-label="关闭"
        >
          <X size={14} />
        </button>
      </div>
    </header>
  );
}
