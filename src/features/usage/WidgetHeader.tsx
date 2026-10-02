import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { WidgetMode } from "./types";

export type { WidgetMode };

export default function WidgetHeader({
  mode,
  onChangeMode,
  onOpenSettings,
  onMenuOpenChange,
}: {
  mode: WidgetMode;
  onChangeMode: (mode: WidgetMode) => void;
  onOpenSettings: () => void;
  onMenuOpenChange: (open: boolean) => void;
}) {
  const [open, setOpen] = useState(false);
  const root = useRef<HTMLDivElement>(null);

  useEffect(() => {
    onMenuOpenChange(open);
    if (!open) return;
    const dismiss = (event: PointerEvent) => {
      if (!root.current?.contains(event.target as Node)) setOpen(false);
    };
    window.addEventListener("pointerdown", dismiss);
    return () => window.removeEventListener("pointerdown", dismiss);
  }, [open, onMenuOpenChange]);

  function changeMode(value: WidgetMode) {
    setOpen(false);
    onChangeMode(value);
  }

  return (
    <header className="widget-header" data-tauri-drag-region>
      <div className="widget-brand" data-tauri-drag-region>
        <div className="widget-brand-icon" aria-hidden="true"><img src="/logo.png" alt="" /></div>
        <div data-tauri-drag-region>
          <h1>tok.io</h1>
          <span>Seu uso de IA</span>
        </div>
      </div>
      <div className="widget-menu-anchor" ref={root}>
        <button className="widget-menu-trigger" aria-label="Abrir menu do widget" aria-haspopup="menu" aria-expanded={open} onClick={() => setOpen((value) => !value)}>
          <svg viewBox="0 0 24 24" aria-hidden="true"><circle cx="5" cy="12" r="1.7" /><circle cx="12" cy="12" r="1.7" /><circle cx="19" cy="12" r="1.7" /></svg>
        </button>
        {open && (
          <div className="widget-menu" role="menu" aria-label="Configurações do widget">
            <span className="widget-menu-label">Exibição</span>
            {([["large", "Painel grande"], ["compact", "Compacto"]] as const).map(([value, label]) => (
              <button key={value} role="menuitemradio" aria-checked={mode === value} onClick={() => changeMode(value)}>{label}<span>{mode === value ? "✓" : ""}</span></button>
            ))}
            <div className="widget-menu-divider" />
            <button role="menuitem" onClick={() => { setOpen(false); onOpenSettings(); }}>Configurações</button>
            <button role="menuitem" onClick={() => void invoke("exit_app")}>Sair do tok.io</button>
          </div>
        )}
      </div>
    </header>
  );
}
