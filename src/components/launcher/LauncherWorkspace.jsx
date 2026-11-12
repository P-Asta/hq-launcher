import { useEffect, useState } from "react";
import { cn } from "../../lib/cn";
import { Search } from "lucide-react";
import { Input } from "../ui/input";

// Layout depends on available width, independently of the runtime platform.
export function useCompactLauncher() {
  const [compact, setCompact] = useState(() => window.matchMedia("(max-width: 799px)").matches);
  useEffect(() => {
    const query = window.matchMedia("(max-width: 799px)");
    const update = () => setCompact(query.matches);
    update();
    query.addEventListener("change", update);
    return () => query.removeEventListener("change", update);
  }, []);
  return compact;
}

export function LauncherWorkspace({ children, className, ...props }) {
  return <div data-launcher-workspace className={cn("mx-auto flex h-full min-h-0 w-full max-w-[1600px] flex-col gap-4 p-4 max-[799px]:gap-3 max-[799px]:p-3", className)} {...props}>{children}</div>;
}

export function LauncherToolbar({ children, className, ...props }) {
  return <div data-launcher-toolbar className={cn("flex flex-wrap items-center gap-3 max-[799px]:gap-2 [&>button]:shrink-0", className)} {...props}>{children}</div>;
}

export function LauncherSearch({ value, onChange }) {
  return <div className="relative ml-2 min-w-44 flex-1 max-[799px]:order-last max-[799px]:ml-0 max-[799px]:basis-full">
    <Search className="pointer-events-none absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-white/40" aria-hidden="true" />
    <Input value={value} onChange={onChange} placeholder="Search mods" aria-label="Search mods" className="h-11 bg-[var(--theme-surface)] pl-10" />
  </div>;
}

export function LauncherModRow({ children, selected, className, ...props }) {
  return <div data-launcher-mod-row className={cn("group flex w-full items-center gap-3 rounded-2xl border border-panel-outline px-3 py-3 text-left transition", selected ? "bg-white/[0.08]" : "bg-black/20 hover:bg-white/[0.07]", className)} {...props}>{children}</div>;
}
