import { useEffect, useState, useRef, useCallback } from "react";
import {
  Plus,
  Trash2,
  Check,
  FolderCog,
  Edit3,
  Loader2,
  AlertCircle,
  FileUp,
  FileDown,
  GripVertical,
} from "lucide-react";
import { useAppStore, type Profile } from "../../stores/appStore";
import { save, open } from "@tauri-apps/plugin-dialog";

export default function ConfigManager() {
  const config = useAppStore((s) => s.config);
  const saveConfig = useAppStore((s) => s.saveConfig);
  const switchProfile = useAppStore((s) => s.switchProfile);
  const exportConfig = useAppStore((s) => s.exportConfig);
  const importConfig = useAppStore((s) => s.importConfig);
  const exportWhistleRules = useAppStore((s) => s.exportWhistleRules);
  const importWhistleRules = useAppStore((s) => s.importWhistleRules);
  const whistleStatus = useAppStore((s) => s.whistleStatus);

  const [newProfileName, setNewProfileName] = useState("");
  const [showNewProfile, setShowNewProfile] = useState(false);
  const [editingId, setEditingId] = useState<string | null>(null);
  const [editName, setEditName] = useState("");
  const [confirmDeleteId, setConfirmDeleteId] = useState<string | null>(null);
  const [opLoading, setOpLoading] = useState<string | null>(null);
  const [opResult, setOpResult] = useState<{ type: "success" | "error"; msg: string } | null>(null);
  const [dragState, setDragState] = useState<{
    dragging: boolean;
    fromIdx: number;
    overIdx: number | null;
    overPos: "above" | "below" | null;
  } | null>(null);
  const profileRefs = useRef<(HTMLDivElement | null)[]>([]);
  const deleteTimerRef = useRef<ReturnType<typeof setTimeout>>();

  useEffect(() => {
    return () => {
      if (deleteTimerRef.current) clearTimeout(deleteTimerRef.current);
    };
  }, []);

  useEffect(() => {
    if (opResult) {
      const t = setTimeout(() => setOpResult(null), 4000);
      return () => clearTimeout(t);
    }
  }, [opResult]);

  const handleCreateProfile = async () => {
    if (!newProfileName.trim() || !config) return;
    const newProfile: Profile = {
      id: `profile-${Date.now()}`,
      name: newProfileName.trim(),
      rules: [],
    };
    await saveConfig({
      ...config,
      profiles: [...config.profiles, newProfile],
    });
    setNewProfileName("");
    setShowNewProfile(false);
  };

  const handleDeleteProfile = async (id: string) => {
    if (confirmDeleteId !== id) {
      setConfirmDeleteId(id);
      if (deleteTimerRef.current) clearTimeout(deleteTimerRef.current);
      deleteTimerRef.current = setTimeout(() => setConfirmDeleteId(null), 3000);
      return;
    }
    setConfirmDeleteId(null);
    if (!config || config.profiles.length <= 1) return;
    const newProfiles = config.profiles.filter((p) => p.id !== id);
    const newActiveId =
      config.active_profile_id === id ? newProfiles[0].id : config.active_profile_id;
    await saveConfig({
      ...config,
      profiles: newProfiles,
      active_profile_id: newActiveId,
    });
  };

  const handleRenameProfile = async (id: string) => {
    if (!config || !editName.trim()) {
      setEditingId(null);
      setEditName("");
      return;
    }
    await saveConfig({
      ...config,
      profiles: config.profiles.map((p) => (p.id === id ? { ...p, name: editName.trim() } : p)),
    });
    setEditingId(null);
    setEditName("");
  };

  const dragCleanupRef = useRef<(() => void) | null>(null);
  const dragStateRef = useRef(dragState);
  dragStateRef.current = dragState;

  useEffect(() => {
    return () => {
      dragCleanupRef.current?.();
    };
  }, []);

  const startDrag = useCallback(
    (fromIdx: number) => {
      setDragState({ dragging: true, fromIdx, overIdx: null, overPos: null });

      const handleMouseMove = (e: MouseEvent) => {
        const refs = profileRefs.current;
        let closestIdx: number | null = null;
        let closestPos: "above" | "below" | null = null;

        for (let i = 0; i < refs.length; i++) {
          const el = refs[i];
          if (!el || i === fromIdx) continue;
          const rect = el.getBoundingClientRect();
          if (e.clientY >= rect.top && e.clientY <= rect.bottom) {
            closestIdx = i;
            closestPos = e.clientY < rect.top + rect.height / 2 ? "above" : "below";
            break;
          }
        }
        setDragState((prev) =>
          prev ? { ...prev, overIdx: closestIdx, overPos: closestPos } : null,
        );
      };

      const cleanup = () => {
        document.removeEventListener("mousemove", handleMouseMove);
        document.removeEventListener("mouseup", handleMouseUp);
        dragCleanupRef.current = null;
      };

      const handleMouseUp = async () => {
        cleanup();

        const prev = dragStateRef.current;
        const latestConfig = useAppStore.getState().config;
        if (prev && prev.overIdx !== null && prev.overIdx !== prev.fromIdx && latestConfig) {
          const targetIdx = prev.overPos === "below" ? prev.overIdx + 1 : prev.overIdx;
          const adjustedTarget = targetIdx > prev.fromIdx ? targetIdx - 1 : targetIdx;
          const newProfiles = [...latestConfig.profiles];
          const [moved] = newProfiles.splice(prev.fromIdx, 1);
          newProfiles.splice(adjustedTarget, 0, moved);
          try {
            await saveConfig({ ...latestConfig, profiles: newProfiles });
          } catch (e) {
            console.error("Failed to save profile reorder:", e);
          }
        }
        setDragState(null);
      };

      document.addEventListener("mousemove", handleMouseMove);
      document.addEventListener("mouseup", handleMouseUp);
      dragCleanupRef.current = cleanup;
    },
    [saveConfig],
  );

  const withFeedback = async (key: string, fn: () => Promise<void>, successMsg: string) => {
    setOpLoading(key);
    setOpResult(null);
    try {
      await fn();
      setOpResult({ type: "success", msg: successMsg });
    } catch (e) {
      const msg = String(e);
      if (msg.includes("__cancelled__")) {
        // User cancelled the dialog, no feedback needed
      } else {
        setOpResult({ type: "error", msg });
      }
    } finally {
      setOpLoading(null);
    }
  };

  const timestamp = () => {
    const d = new Date();
    return `${d.getFullYear()}${String(d.getMonth() + 1).padStart(2, "0")}${String(d.getDate()).padStart(2, "0")}_${String(d.getHours()).padStart(2, "0")}${String(d.getMinutes()).padStart(2, "0")}`;
  };

  const handleExportAppConfig = () =>
    withFeedback(
      "export-config",
      async () => {
        const path = await save({
          filters: [{ name: "JSON", extensions: ["json"] }],
          defaultPath: `whistlebox-config_${timestamp()}.json`,
        });
        if (!path) throw new Error("__cancelled__");
        await exportConfig(path);
      },
      "应用配置已导出",
    );

  const handleImportAppConfig = () =>
    withFeedback(
      "import-config",
      async () => {
        const path = await open({
          filters: [{ name: "JSON", extensions: ["json"] }],
          multiple: false,
        });
        if (!path || typeof path !== "string") throw new Error("__cancelled__");
        await importConfig(path);
      },
      "应用配置已导入",
    );

  const whistleAlive = whistleStatus?.running && whistleStatus?.uptime_check;

  const handleExportWhistleRules = () =>
    withFeedback(
      "export-rules",
      async () => {
        if (!whistleAlive) {
          throw new Error("Whistle 未运行，无法导出规则。请先启动 Whistle。");
        }
        const path = await save({
          filters: [{ name: "Text", extensions: ["txt", "json"] }],
          defaultPath: `whistle-rules_${timestamp()}.txt`,
        });
        if (!path) throw new Error("__cancelled__");
        await exportWhistleRules(path);
      },
      "Whistle 规则已导出",
    );

  const handleImportWhistleRules = () =>
    withFeedback(
      "import-rules",
      async () => {
        if (!whistleAlive) {
          throw new Error("Whistle 未运行，无法导入规则。请先启动 Whistle。");
        }
        const path = await open({
          filters: [{ name: "Text", extensions: ["txt", "json"] }],
          multiple: false,
        });
        if (!path || typeof path !== "string") throw new Error("__cancelled__");
        await importWhistleRules(path);
      },
      "Whistle 规则已导入",
    );

  return (
    <div className="animate-fade-in">
      <div className="sticky top-0 z-10 bg-[var(--bg-main)] px-6 pt-6 pb-3 border-b border-[var(--border-color)]">
        <div className="max-w-5xl mx-auto">
          <h1 className="text-2xl font-bold themed-text tracking-tight">配置管理</h1>
          <p className="text-sm themed-text-muted mt-1">管理配置文件、导入导出规则</p>
        </div>
      </div>

      <div className="px-6 pb-6 pt-4 space-y-6 max-w-5xl mx-auto">
        {opResult && (
          <div
            className={`glass-panel p-3 border-l-[3px] flex items-center gap-3 animate-fade-in ${opResult.type === "success" ? "border-l-accent-500" : "border-l-danger-500"
              }`}
          >
            {opResult.type === "success" ? (
              <Check size={16} className="text-accent-400 shrink-0" />
            ) : (
              <AlertCircle size={16} className="text-danger-400 shrink-0" />
            )}
            <span
              className={`text-xs ${opResult.type === "success" ? "text-accent-400" : "text-danger-400"}`}
            >
              {opResult.msg}
            </span>
          </div>
        )}

        {/* Profiles */}
        <div>
          <div className="flex items-center justify-between mb-3">
            <h2 className="text-sm font-semibold themed-text-secondary uppercase tracking-wider">
              配置文件
            </h2>
            <button
              onClick={() => setShowNewProfile(!showNewProfile)}
              className="btn-secondary text-xs flex items-center gap-1.5 py-1.5"
            >
              <Plus size={12} />
              新建
            </button>
          </div>

          {showNewProfile && (
            <div className="glass-panel p-3 mb-3 flex gap-2 animate-fade-in">
              <input
                type="text"
                value={newProfileName}
                onChange={(e) => setNewProfileName(e.target.value)}
                onKeyDown={(e) => e.key === "Enter" && handleCreateProfile()}
                placeholder="配置名称"
                className="input-field flex-1 text-sm"
                autoFocus
              />
              <button onClick={handleCreateProfile} className="btn-primary text-xs py-1.5">
                创建
              </button>
            </div>
          )}

          <div className="space-y-2">
            {config?.profiles?.map((profile, idx) => {
              const isActive = profile.id === config?.active_profile_id;
              const isEditing = editingId === profile.id;

              const isDragging = dragState?.fromIdx === idx;
              const isOver = dragState?.overIdx === idx;
              const showAbove = isOver && dragState?.overPos === "above";
              const showBelow = isOver && dragState?.overPos === "below";

              return (
                <div
                  key={profile.id}
                  className="relative"
                  ref={(el) => {
                    profileRefs.current[idx] = el;
                  }}
                >
                  {showAbove && (
                    <div className="absolute -top-1.5 left-4 right-4 h-0.5 bg-accent-400 rounded-full z-10 shadow-[0_0_6px_rgba(23,179,127,0.5)]" />
                  )}
                  <div
                    onClick={() => {
                      if (!isActive && !isEditing && !dragState) switchProfile(profile.id);
                    }}
                    className={`flex items-center gap-3 p-3.5 rounded-xl border transition-all duration-200 ${isActive
                      ? "bg-accent-950/30 border-accent-800/20 ring-1 ring-accent-700/20"
                      : "bg-surface-900/30 border-surface-800/50 hover:border-surface-700/50 cursor-pointer"
                      } ${isDragging ? "opacity-50" : ""}`}
                  >
                    <div
                      className="text-surface-600 cursor-grab active:cursor-grabbing shrink-0"
                      onMouseDown={(e) => {
                        e.preventDefault();
                        e.stopPropagation();
                        startDrag(idx);
                      }}
                    >
                      <GripVertical size={14} />
                    </div>
                    <div
                      className={`w-9 h-9 rounded-lg flex items-center justify-center shrink-0 ${isActive
                        ? "bg-accent-500/15 text-accent-400"
                        : "bg-surface-800 text-surface-500"
                        }`}
                    >
                      <FolderCog size={16} />
                    </div>
                    <div className="flex-1 min-w-0">
                      {isEditing ? (
                        <input
                          type="text"
                          value={editName}
                          onChange={(e) => setEditName(e.target.value)}
                          onKeyDown={(e) => {
                            if (e.key === "Enter") handleRenameProfile(profile.id);
                            if (e.key === "Escape") setEditingId(null);
                          }}
                          onBlur={() => handleRenameProfile(profile.id)}
                          onClick={(e) => e.stopPropagation()}
                          className="input-field text-sm w-full py-1"
                          autoFocus
                        />
                      ) : (
                        <>
                          <div className="text-sm font-medium themed-text">{profile.name}</div>
                          <div className="text-[11px] themed-text-muted">
                            {profile.rules.length} 条规则 ·{" "}
                            {profile.rules.filter((r) => r.enabled).length} 条启用
                          </div>
                        </>
                      )}
                    </div>
                    <div className="flex items-center gap-1.5" onClick={(e) => e.stopPropagation()}>
                      {isActive && (
                        <span className="text-xs px-2 py-0.5 rounded-full bg-accent-500/15 text-accent-400 flex items-center gap-1">
                          <Check size={10} />
                          当前
                        </span>
                      )}
                      <button
                        onClick={() => {
                          if (editingId === profile.id) {
                            setEditingId(null);
                            setEditName("");
                          } else {
                            setEditingId(profile.id);
                            setEditName(profile.name);
                          }
                        }}
                        className={`p-1 rounded-md transition-colors ${isEditing ? "text-accent-400" : "text-surface-600 hover:text-surface-300"
                          }`}
                      >
                        <Edit3 size={12} />
                      </button>
                      {config.profiles.length > 1 && (
                        <button
                          onClick={() => handleDeleteProfile(profile.id)}
                          className={`p-1 rounded-md transition-colors ${confirmDeleteId === profile.id
                            ? "text-danger-400 bg-danger-500/15"
                            : "text-surface-600 hover:text-danger-400"
                            }`}
                          title={confirmDeleteId === profile.id ? "再次点击确认删除" : "删除"}
                        >
                          <Trash2 size={12} />
                        </button>
                      )}
                    </div>
                  </div>
                  {showBelow && (
                    <div className="absolute -bottom-1.5 left-4 right-4 h-0.5 bg-accent-400 rounded-full z-10 shadow-[0_0_6px_rgba(23,179,127,0.5)]" />
                  )}
                </div>
              );
            })}
          </div>
        </div>

        {/* Import/Export */}
        <div>
          <h2 className="text-sm font-semibold themed-text-secondary uppercase tracking-wider mb-3">
            应用配置
          </h2>
          <div className="grid grid-cols-2 gap-3">
            <ActionCard
              icon={<FileDown size={18} />}
              label="导出应用配置"
              sublabel="导出所有设置与规则"
              onClick={handleExportAppConfig}
              loading={opLoading === "export-config"}
              color="accent"
            />
            <ActionCard
              icon={<FileUp size={18} />}
              label="导入应用配置"
              sublabel="从文件恢复配置"
              onClick={handleImportAppConfig}
              loading={opLoading === "import-config"}
              color="accent"
            />
          </div>
        </div>

        <div>
          <h2 className="text-sm font-semibold themed-text-secondary uppercase tracking-wider mb-3">
            Whistle 规则
          </h2>
          {!whistleAlive && (
            <div className="glass-panel p-3 mb-3 border-l-[3px] border-l-warning-500 flex items-center gap-3">
              <AlertCircle size={14} className="text-warning-400 shrink-0" />
              <span className="text-xs text-warning-400">
                Whistle 未运行，规则导入导出需要先启动 Whistle
              </span>
            </div>
          )}
          <div className="grid grid-cols-2 gap-3">
            <ActionCard
              icon={<FileDown size={18} />}
              label="导出 Whistle 规则"
              sublabel="从 Whistle 实例导出"
              onClick={handleExportWhistleRules}
              loading={opLoading === "export-rules"}
              disabled={!whistleAlive}
              color="blue"
            />
            <ActionCard
              icon={<FileUp size={18} />}
              label="导入 Whistle 规则"
              sublabel="导入到 Whistle 实例"
              onClick={handleImportWhistleRules}
              loading={opLoading === "import-rules"}
              disabled={!whistleAlive}
              color="blue"
            />
          </div>
        </div>
      </div>
    </div>
  );
}

function ActionCard({
  icon,
  label,
  sublabel,
  onClick,
  loading,
  disabled,
  color,
}: {
  icon: React.ReactNode;
  label: string;
  sublabel: string;
  onClick: () => void;
  loading?: boolean;
  disabled?: boolean;
  color: "accent" | "blue";
}) {
  const colorClasses =
    color === "accent"
      ? "bg-accent-500/10 text-accent-400 hover:border-accent-800/30"
      : "bg-blue-500/10 text-blue-400 hover:border-blue-800/30";

  return (
    <button
      onClick={onClick}
      disabled={loading || disabled}
      className={`card flex items-center gap-3 active:scale-[0.98] ${disabled ? "opacity-50 cursor-not-allowed" : ""
        }`}
    >
      <div className={`w-10 h-10 rounded-xl flex items-center justify-center ${colorClasses}`}>
        {loading ? <Loader2 size={18} className="animate-spin" /> : icon}
      </div>
      <div className="text-left">
        <div className="text-sm font-medium themed-text">{label}</div>
        <div className="text-[11px] themed-text-muted">{sublabel}</div>
      </div>
    </button>
  );
}
