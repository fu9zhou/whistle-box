import { useEffect, useState, useRef, useCallback } from "react";
import { Plus, Trash2, ToggleLeft, ToggleRight, Search, AlertCircle } from "lucide-react";
import { useAppStore, type ProxyRule, type AppConfig } from "../../stores/appStore";

export default function RuleEditor() {
  const config = useAppStore((s) => s.config);
  const saveConfig = useAppStore((s) => s.saveConfig);
  const proxyStatus = useAppStore((s) => s.proxyStatus);
  const refreshPac = useAppStore((s) => s.refreshPac);
  const [search, setSearch] = useState("");
  const [newPattern, setNewPattern] = useState("");
  const [newComment, setNewComment] = useState("");
  const [localRules, setLocalRules] = useState<ProxyRule[]>([]);
  const [confirmDeleteId, setConfirmDeleteId] = useState<string | null>(null);
  const configLoadedRef = useRef(false);
  const configRef = useRef(config);
  configRef.current = config;
  const saveTimerRef = useRef<ReturnType<typeof setTimeout>>();
  const deleteTimerRef = useRef<ReturnType<typeof setTimeout>>();
  const savePendingRef = useRef(false);
  const saveQueueRef = useRef<Promise<void>>(Promise.resolve());
  const pendingSaveCountRef = useRef(0);
  const saveRevisionRef = useRef(0);

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

  useEffect(() => {
    return () => {
      if (saveTimerRef.current) clearTimeout(saveTimerRef.current);
      if (deleteTimerRef.current) clearTimeout(deleteTimerRef.current);
    };
  }, []);

  useEffect(() => {
    if (config && !savePendingRef.current) {
      const profile = config.profiles.find((p) => p.id === config.active_profile_id);
      setLocalRules(profile?.rules ?? []);
      configLoadedRef.current = true;
    }
  }, [config]);

  const autoSaveRules = useCallback(
    (rules: ProxyRule[]) => {
      if (!configRef.current || !configLoadedRef.current) return;
      if (saveTimerRef.current) clearTimeout(saveTimerRef.current);
      const revision = ++saveRevisionRef.current;
      const targetProfileId = configRef.current.active_profile_id;
      saveTimerRef.current = setTimeout(async () => {
        enqueueSave(async () => {
          const latestConfig = configRef.current;
          if (!latestConfig) return;
          if (revision !== saveRevisionRef.current) return;
          if (latestConfig.active_profile_id !== targetProfileId) return;
          const newConfig: AppConfig = {
            ...latestConfig,
            profiles: latestConfig.profiles.map((p) =>
              p.id === targetProfileId ? { ...p, rules } : p,
            ),
          };
          await saveConfig(newConfig);
          if (revision !== saveRevisionRef.current) return;
          await refreshPac();
        });
      }, 800);
    },
    [saveConfig, refreshPac, enqueueSave],
  );

  const filteredRules = localRules.filter(
    (r) =>
      r.pattern.toLowerCase().includes(search.toLowerCase()) ||
      r.comment.toLowerCase().includes(search.toLowerCase()),
  );

  const addRule = () => {
    if (!newPattern.trim()) return;
    const rule: ProxyRule = {
      id: `rule-${Date.now()}`,
      pattern: newPattern.trim(),
      enabled: true,
      comment: newComment.trim(),
    };
    const newRules = [...localRules, rule];
    setLocalRules(newRules);
    setNewPattern("");
    setNewComment("");
    autoSaveRules(newRules);
  };

  const removeRule = (id: string) => {
    if (confirmDeleteId !== id) {
      setConfirmDeleteId(id);
      if (deleteTimerRef.current) clearTimeout(deleteTimerRef.current);
      deleteTimerRef.current = setTimeout(() => setConfirmDeleteId(null), 3000);
      return;
    }
    setConfirmDeleteId(null);
    const newRules = localRules.filter((r) => r.id !== id);
    setLocalRules(newRules);
    autoSaveRules(newRules);
  };

  const toggleRule = (id: string) => {
    const newRules = localRules.map((r) => (r.id === id ? { ...r, enabled: !r.enabled } : r));
    setLocalRules(newRules);
    autoSaveRules(newRules);
  };

  return (
    <div className="animate-fade-in">
      <div className="sticky top-0 z-10 bg-[var(--bg-main)] px-6 pt-6 pb-3 border-b border-[var(--border-color)]">
        <div className="flex items-center justify-between max-w-5xl mx-auto">
          <div>
            <h1 className="text-2xl font-bold themed-text tracking-tight">规则编辑</h1>
            <p className="text-sm themed-text-muted mt-1">
              配置域名匹配规则，规则模式下仅匹配的请求走代理
            </p>
          </div>
          {proxyStatus && (
            <div
              className={`flex items-center gap-2 px-3 py-1.5 rounded-full text-xs font-medium ${
                proxyStatus.mode === "rule"
                  ? "bg-blue-500/10 text-blue-400"
                  : "bg-surface-800/50 text-surface-500"
              }`}
            >
              <span
                className={`w-1.5 h-1.5 rounded-full ${
                  proxyStatus.mode === "rule" ? "bg-blue-400" : "bg-surface-500"
                }`}
              />
              {proxyStatus.mode === "rule" ? "规则模式已启用" : "当前非规则模式"}
            </div>
          )}
        </div>
      </div>

      <div className="px-6 pb-6 pt-4 space-y-5 max-w-5xl mx-auto">
        {/* Add new rule */}
        <div className="glass-panel p-4">
          <div className="text-xs font-semibold text-surface-400 uppercase tracking-wider mb-3">
            添加规则
          </div>
          <div className="flex gap-3">
            <input
              type="text"
              value={newPattern}
              onChange={(e) => setNewPattern(e.target.value)}
              onKeyDown={(e) => e.key === "Enter" && addRule()}
              placeholder="域名模式，如 *.example.com"
              className="input-field flex-1 font-mono text-sm"
            />
            <input
              type="text"
              value={newComment}
              onChange={(e) => setNewComment(e.target.value)}
              onKeyDown={(e) => e.key === "Enter" && addRule()}
              placeholder="备注（可选）"
              className="input-field w-48 text-sm"
            />
            <button onClick={addRule} className="btn-primary flex items-center gap-1.5">
              <Plus size={14} />
              添加
            </button>
          </div>
          <div className="mt-2 flex items-start gap-1.5 text-[11px] text-surface-500">
            <AlertCircle size={12} className="mt-0.5 shrink-0" />
            <span>
              支持通配符 <code className="text-accent-400 font-mono">*</code>。 使用{" "}
              <code className="text-accent-400 font-mono">*.example.com</code> 匹配所有 example.com
              的子域名，使用 <code className="text-accent-400 font-mono">docs.example.com</code>{" "}
              精确匹配单个域名
            </span>
          </div>
        </div>

        {/* Search */}
        <div className="relative">
          <Search size={14} className="absolute left-3 top-1/2 -translate-y-1/2 text-surface-500" />
          <input
            type="text"
            value={search}
            onChange={(e) => setSearch(e.target.value)}
            placeholder="搜索规则..."
            className="input-field w-full pl-9 text-sm"
          />
        </div>

        {/* Rules list */}
        <div className="space-y-1.5">
          {filteredRules.length === 0 ? (
            <div className="glass-panel p-8 text-center">
              <div className="text-surface-600 text-sm">
                {localRules.length === 0
                  ? "暂无规则，添加域名模式开始使用规则代理"
                  : "没有匹配的规则"}
              </div>
            </div>
          ) : (
            filteredRules.map((rule, idx) => (
              <div
                key={rule.id}
                className={`flex items-center gap-3 p-3 rounded-lg border transition-all duration-200 ${
                  rule.enabled
                    ? "bg-surface-900/30 border-surface-800/50"
                    : "bg-surface-950/50 border-surface-900/30 opacity-60"
                }`}
                style={{ animationDelay: `${idx * 30}ms` }}
              >
                <button
                  onClick={() => toggleRule(rule.id)}
                  className="shrink-0 text-surface-400 hover:text-accent-400 transition-colors"
                >
                  {rule.enabled ? (
                    <ToggleRight size={22} className="text-accent-500" />
                  ) : (
                    <ToggleLeft size={22} />
                  )}
                </button>
                <div className="flex-1 min-w-0">
                  <div className="font-mono text-sm text-surface-200 truncate">{rule.pattern}</div>
                  {rule.comment && (
                    <div className="text-[11px] text-surface-500 mt-0.5 truncate">
                      {rule.comment}
                    </div>
                  )}
                </div>
                <button
                  onClick={() => removeRule(rule.id)}
                  className={`shrink-0 p-1.5 rounded-md transition-all ${
                    confirmDeleteId === rule.id
                      ? "text-danger-400 bg-danger-500/15"
                      : "text-surface-600 hover:text-danger-400 hover:bg-danger-500/10"
                  }`}
                  title={confirmDeleteId === rule.id ? "再次点击确认删除" : "删除"}
                >
                  <Trash2 size={14} />
                </button>
              </div>
            ))
          )}
        </div>

        {localRules.length > 0 && (
          <div className="text-xs text-surface-600 text-center">
            共 {localRules.length} 条规则，
            {localRules.filter((r) => r.enabled).length} 条启用
          </div>
        )}
      </div>
    </div>
  );
}
