import { Info } from "lucide-react";

export function Section({
  icon,
  title,
  action,
  children,
}: {
  icon: React.ReactNode;
  title: string;
  action?: React.ReactNode;
  children: React.ReactNode;
}) {
  return (
    <div className="glass-panel p-5 space-y-4 break-inside-avoid">
      <div className="flex items-center justify-between mb-1">
        <div className="flex items-center gap-2">
          <span className="text-accent-400">{icon}</span>
          <h2 className="text-sm font-semibold themed-text-secondary">{title}</h2>
        </div>
        {action}
      </div>
      {children}
    </div>
  );
}

export function Field({
  label,
  hint,
  htmlFor,
  children,
}: {
  label: string;
  hint?: string;
  htmlFor?: string;
  children: React.ReactNode;
}) {
  return (
    <div>
      <label htmlFor={htmlFor} className="block text-xs themed-text-muted mb-1.5">
        {label}
      </label>
      {children}
      {hint && <div className="text-[11px] themed-text-muted mt-1 opacity-70">{hint}</div>}
    </div>
  );
}

export function Label({ children }: { children: React.ReactNode }) {
  return <label className="block text-xs themed-text-muted mb-1.5">{children}</label>;
}

export function Hint({ children }: { children: React.ReactNode }) {
  return (
    <div className="flex items-start gap-1.5 text-[11px] themed-text-muted">
      <Info size={12} className="mt-0.5 shrink-0" />
      <span>{children}</span>
    </div>
  );
}

export function Toggle({
  checked,
  onChange,
  label,
  sublabel,
}: {
  checked: boolean;
  onChange: (v: boolean) => void;
  label: string;
  sublabel: React.ReactNode;
}) {
  return (
    <label className="flex items-center gap-3 cursor-pointer group">
      <div className="relative">
        <input
          type="checkbox"
          checked={checked}
          onChange={(e) => onChange(e.target.checked)}
          className="sr-only peer"
        />
        <div className="w-9 h-5 rounded-full bg-surface-700 peer-checked:bg-accent-600 transition-colors" />
        <div className="absolute top-0.5 left-0.5 w-4 h-4 rounded-full bg-white shadow-sm peer-checked:translate-x-4 transition-transform" />
      </div>
      <div>
        <div className="text-sm themed-text-secondary group-hover:themed-text transition-colors">
          {label}
        </div>
        <div className="text-[11px] themed-text-muted">{sublabel}</div>
      </div>
    </label>
  );
}

export function ModeButton({
  active,
  onClick,
  color,
  label,
  sublabel,
}: {
  active: boolean;
  onClick: () => void;
  color: "accent" | "blue";
  label: string;
  sublabel: string;
}) {
  const activeClass =
    color === "accent"
      ? "bg-accent-950/40 border-accent-700/30 text-accent-400"
      : "bg-blue-950/40 border-blue-700/30 text-blue-400";

  return (
    <button
      onClick={onClick}
      className={`flex-1 p-3 rounded-lg border text-sm text-center transition-all ${
        active
          ? activeClass
          : "bg-surface-900/50 border-surface-800 themed-text-secondary hover:border-surface-700"
      }`}
    >
      <div className="font-medium">{label}</div>
      <div className="text-[11px] mt-0.5 opacity-70">{sublabel}</div>
    </button>
  );
}
