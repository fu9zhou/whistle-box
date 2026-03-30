import { useState, useEffect, lazy, Suspense } from "react";
import Sidebar from "./components/layout/Sidebar";
import TitleBar from "./components/layout/TitleBar";
import Dashboard from "./components/dashboard/Dashboard";
import { useAppStore } from "./stores/appStore";
import { X } from "lucide-react";
import { VALID_PAGES, type Page } from "./types";

const ProxyControl = lazy(() => import("./components/proxy/ProxyControl"));
const RuleEditor = lazy(() => import("./components/rules/RuleEditor"));
const WhistleView = lazy(() => import("./components/whistle/WhistleView"));
const ConfigManager = lazy(() => import("./components/config/ConfigManager"));
const Settings = lazy(() => import("./components/settings/Settings"));
const SetupWizard = lazy(() => import("./components/setup/SetupWizard"));

export default function App() {
  const [currentPage, setCurrentPage] = useState<Page>("dashboard");
  const config = useAppStore((s) => s.config);
  const error = useAppStore((s) => s.error);
  const clearError = useAppStore((s) => s.clearError);
  const theme = useAppStore((s) => s.theme);
  const loading = useAppStore((s) => s.loading);
  const loadConfig = useAppStore((s) => s.loadConfig);
  const [setupDone, setSetupDone] = useState<boolean | null>(null);

  useEffect(() => {
    if (theme === "light") {
      document.documentElement.classList.add("light");
    } else {
      document.documentElement.classList.remove("light");
    }
  }, [theme]);

  useEffect(() => {
    loadConfig();
  }, [loadConfig]);

  useEffect(() => {
    if (config) {
      setSetupDone(config.setup_completed === true);
    } else if (!loading) {
      setSetupDone(false);
    }
  }, [config, loading]);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    let mounted = true;
    import("@tauri-apps/api/event")
      .then(({ listen }) => {
        listen<string>("navigate", (event) => {
          const payload = event.payload;
          if ((VALID_PAGES as readonly string[]).includes(payload)) {
            setCurrentPage(payload as Page);
          }
        }).then((fn) => {
          if (mounted) unlisten = fn;
          else fn();
        });
      })
      .catch(() => {});
    return () => {
      mounted = false;
      unlisten?.();
    };
  }, []);

  useEffect(() => {
    if (error) {
      const timer = setTimeout(clearError, 8000);
      return () => clearTimeout(timer);
    }
  }, [error, clearError]);

  const renderPage = () => {
    switch (currentPage) {
      case "dashboard":
        return <Dashboard onNavigate={setCurrentPage} />;
      case "proxy":
        return <ProxyControl />;
      case "rules":
        return <RuleEditor />;
      case "whistle":
        return <WhistleView />;
      case "config":
        return <ConfigManager />;
      case "settings":
        return <Settings />;
      default:
        return <Dashboard onNavigate={setCurrentPage} />;
    }
  };

  if (setupDone === null) {
    return (
      <div className="h-screen w-screen flex flex-col items-center justify-center themed-main">
        <TitleBar />
        <div className="flex-1 flex items-center justify-center">
          <div className="w-6 h-6 border-2 border-[var(--accent-500)] border-t-transparent rounded-full animate-spin" />
        </div>
      </div>
    );
  }

  if (setupDone === false) {
    return (
      <div className="h-screen w-screen flex flex-col overflow-hidden themed-main">
        <TitleBar />
        <Suspense fallback={null}>
          <SetupWizard
            onComplete={() => {
              setSetupDone(true);
              loadConfig();
            }}
          />
        </Suspense>
      </div>
    );
  }

  return (
    <div className="h-screen w-screen flex flex-col overflow-hidden themed-main">
      <TitleBar />
      <div className="flex flex-1 overflow-hidden">
        <Sidebar currentPage={currentPage} onNavigate={setCurrentPage} />
        <main className="flex-1 overflow-y-auto">
          <Suspense
            fallback={
              <div className="flex items-center justify-center h-full">
                <div className="w-6 h-6 border-2 border-[var(--accent-500)] border-t-transparent rounded-full animate-spin" />
              </div>
            }
          >
            {renderPage()}
          </Suspense>
        </main>
      </div>

      {error && (
        <div className="toast-error flex items-start gap-3">
          <span className="flex-1">{error}</span>
          <button onClick={clearError} className="shrink-0 mt-0.5 opacity-70 hover:opacity-100">
            <X size={14} />
          </button>
        </div>
      )}
    </div>
  );
}
