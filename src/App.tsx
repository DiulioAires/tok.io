import { useCallback, useEffect, useRef, useState } from "react";
import type { PointerEvent } from "react";
import { listen } from "@tauri-apps/api/event";
import { PhysicalPosition, LogicalSize } from "@tauri-apps/api/dpi";
import { availableMonitors, cursorPosition, getCurrentWindow, primaryMonitor, type Monitor } from "@tauri-apps/api/window";
import LargeUsageWidget from "./features/usage/LargeUsageWidget";
import CompactUsageWidget from "./features/usage/CompactUsageWidget";
import WidgetSettings from "./features/usage/WidgetSettings";
import { getWidgetPreferences, saveWidgetPosition, setWidgetMode } from "./features/usage/usageApi";
import { useUsageData } from "./features/usage/usageData";
import { widgetPositionFitsDisplay } from "./features/usage/widgetPreferences";
import type { WidgetMode, WidgetPosition } from "./features/usage/types";
import "./features/usage/UsageDashboard.css";
import "./App.css";

type Edge = "top" | "right" | "bottom" | "left";
const FULL_SIZE: Record<"large" | "compact", { width: number; height: number }> = {
  large: { width: 460, height: 430 },
  compact: { width: 520, height: 190 },
};
const TAB = { width: 58, height: 38 };
const COLLAPSE_DELAY = 520;

function clamp(value: number, min: number, max: number) {
  return Math.min(Math.max(value, min), Math.max(min, max));
}

function clampPhysical(value: number, min: number, max: number) {
  return Math.round(clamp(value, min, max));
}

function monitorForWindow(monitors: Monitor[], position: WidgetPosition, width: number, height: number) {
  const centerX = position.x + width / 2;
  const centerY = position.y + height / 2;
  return monitors.find((monitor) => {
    const origin = monitor.position;
    const size = monitor.size;
    return centerX >= origin.x && centerX < origin.x + size.width && centerY >= origin.y && centerY < origin.y + size.height;
  }) ?? monitors[0];
}

function App() {
  const appWindow = getCurrentWindow();
  const label: "large" | "compact" = appWindow.label === "compact" ? "compact" : "large";
  const fullSize = FULL_SIZE[label];
  const usage = useUsageData();
  const [mode, setMode] = useState<WidgetMode>("large");
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [menuOpen, setMenuOpen] = useState(false);
  const [collapsed, setCollapsed] = useState(false);
  const [retracting, setRetracting] = useState(false);
  const [edge, setEdge] = useState<Edge>("top");
  const [dragging, setDragging] = useState(false);
  const pointerInsideRef = useRef(false);
  const collapsedRef = useRef(false);
  const transitionRef = useRef(false);
  const draggingRef = useRef(false);
  const menuOpenRef = useRef(false);
  const settingsOpenRef = useRef(false);
  const collapseTimer = useRef<number | undefined>(undefined);
  const saveTimer = useRef<number | undefined>(undefined);
  const expandedPosition = useRef<WidgetPosition | null>(null);

  const persistPosition = useCallback((position: WidgetPosition) => {
    if (saveTimer.current !== undefined) window.clearTimeout(saveTimer.current);
    saveTimer.current = window.setTimeout(() => void saveWidgetPosition(label, position).catch(() => undefined), 250);
  }, [label]);

  const flushPosition = useCallback(async () => {
    if (collapsedRef.current) return;
    if (saveTimer.current !== undefined) window.clearTimeout(saveTimer.current);
    try {
      const position = await appWindow.outerPosition();
      const saved = { x: position.x, y: position.y };
      expandedPosition.current = saved;
      await saveWidgetPosition(label, saved);
    } catch {
      // The most recent debounced save remains the fallback if the window is closing.
    }
  }, [appWindow, label]);

  const expand = useCallback(async () => {
    if (!collapsedRef.current || transitionRef.current) return;
    transitionRef.current = true;
    if (collapseTimer.current !== undefined) window.clearTimeout(collapseTimer.current);
    let remembered = expandedPosition.current;
    try {
      const monitors = await availableMonitors();
      if (remembered && !monitors.some((monitor) => widgetPositionFitsDisplay(remembered!, fullSize, monitor))) {
        const primary = await primaryMonitor();
        if (primary) {
          remembered = {
            x: primary.position.x + Math.max(0, Math.round((primary.size.width - fullSize.width * primary.scaleFactor) / 2)),
            y: primary.position.y + Math.max(0, Math.round((primary.size.height - fullSize.height * primary.scaleFactor) / 2)),
          };
          expandedPosition.current = remembered;
        }
      }
      await appWindow.setSize(new LogicalSize(fullSize.width, fullSize.height));
      if (remembered) await appWindow.setPosition(new PhysicalPosition(remembered.x, remembered.y));
      if (remembered) persistPosition(remembered);
    } catch (error) {
      console.error("Não foi possível abrir o widget:", error);
    } finally {
      transitionRef.current = false;
    }
    collapsedRef.current = false;
    setCollapsed(false);
  }, [appWindow, fullSize.height, fullSize.width, persistPosition]);

  const collapse = useCallback(async () => {
    if (collapsedRef.current || transitionRef.current || draggingRef.current || menuOpenRef.current || settingsOpenRef.current) return;
    try {
      transitionRef.current = true;
      const [position, size, monitors] = await Promise.all([
        appWindow.outerPosition(),
        appWindow.outerSize(),
        availableMonitors(),
      ]);
      const current = { x: position.x, y: position.y };
      const monitor = monitorForWindow(monitors, current, size.width, size.height);
      if (!monitor) return;
      expandedPosition.current = current;
      persistPosition(current);
      const origin = monitor.position;
      const bounds = monitor.size;
      const right = origin.x + bounds.width;
      const bottom = origin.y + bounds.height;
      const distances: Record<Edge, number> = {
        top: Math.max(0, current.y - origin.y),
        right: Math.max(0, right - (current.x + size.width)),
        bottom: Math.max(0, bottom - (current.y + size.height)),
        left: Math.max(0, current.x - origin.x),
      };
      const nearest = (Object.keys(distances) as Edge[]).reduce((best, candidate) => distances[candidate] < distances[best] ? candidate : best, "top");
      setEdge(nearest);
      setRetracting(true);
      await new Promise<void>((resolve) => window.setTimeout(resolve, 260));
      if (pointerInsideRef.current || menuOpenRef.current || settingsOpenRef.current || draggingRef.current) return;
      const scale = monitor.scaleFactor;
      const tabWidth = Math.round((nearest === "left" || nearest === "right" ? TAB.height : TAB.width) * scale);
      const tabHeight = Math.round((nearest === "left" || nearest === "right" ? TAB.width : TAB.height) * scale);
      let collapsedPosition: WidgetPosition;
      if (nearest === "top") collapsedPosition = { x: clampPhysical(current.x + size.width / 2 - tabWidth / 2, origin.x, right - tabWidth), y: origin.y };
      else if (nearest === "bottom") collapsedPosition = { x: clampPhysical(current.x + size.width / 2 - tabWidth / 2, origin.x, right - tabWidth), y: bottom - tabHeight };
      else if (nearest === "left") collapsedPosition = { x: origin.x, y: clampPhysical(current.y + size.height / 2 - tabHeight / 2, origin.y, bottom - tabHeight) };
      else collapsedPosition = { x: right - tabWidth, y: clampPhysical(current.y + size.height / 2 - tabHeight / 2, origin.y, bottom - tabHeight) };
      await appWindow.setSize(new LogicalSize(nearest === "left" || nearest === "right" ? TAB.height : TAB.width, nearest === "left" || nearest === "right" ? TAB.width : TAB.height));
      await appWindow.setPosition(new PhysicalPosition(collapsedPosition.x, collapsedPosition.y));
      collapsedRef.current = true;
      setCollapsed(true);
    } catch (error) {
      console.error("Não foi possível recolher o widget:", error);
    } finally {
      setRetracting(false);
      transitionRef.current = false;
    }
  }, [appWindow, persistPosition]);

  const scheduleCollapse = useCallback(() => {
    if (collapseTimer.current !== undefined) window.clearTimeout(collapseTimer.current);
    if (draggingRef.current || menuOpenRef.current || settingsOpenRef.current || collapsedRef.current) return;
    collapseTimer.current = window.setTimeout(() => void collapse(), COLLAPSE_DELAY);
  }, [collapse]);

  useEffect(() => {
    let unlistenMove: (() => void) | undefined;
    let unlistenMode: (() => void) | undefined;
    let disposed = false;
    void getWidgetPreferences().then((preferences) => {
      setMode(preferences.mode);
      const saved = label === "large" ? preferences.large_position : preferences.compact_position;
      if (saved) expandedPosition.current = saved;
      else void appWindow.outerPosition().then((position) => {
        expandedPosition.current = { x: position.x, y: position.y };
      });
    }).catch(() => undefined);
    void appWindow.onMoved(({ payload }) => {
      if (!collapsedRef.current && !transitionRef.current) {
        const next = { x: payload.x, y: payload.y };
        expandedPosition.current = next;
        persistPosition(next);
      }
    }).then((unlisten) => {
      if (disposed) unlisten();
      else unlistenMove = unlisten;
    });
    void listen<WidgetMode>("widget-mode-updated", (event) => setMode(event.payload)).then((unlisten) => {
      if (disposed) unlisten();
      else unlistenMode = unlisten;
    });
    const endDrag = () => {
      if (!draggingRef.current) return;
      draggingRef.current = false;
      setDragging(false);
      void flushPosition();
      if (menuOpenRef.current || settingsOpenRef.current) return;
      void Promise.all([cursorPosition(), appWindow.outerPosition(), appWindow.outerSize()]).then(([cursor, position, size]) => {
        const inside = cursor.x >= position.x && cursor.x < position.x + size.width
          && cursor.y >= position.y && cursor.y < position.y + size.height;
        if (!inside) scheduleCollapse();
      }).catch(() => undefined);
    };
    window.addEventListener("pointerup", endDrag);
    return () => {
      disposed = true;
      void flushPosition();
      unlistenMove?.();
      unlistenMode?.();
      window.removeEventListener("pointerup", endDrag);
      if (collapseTimer.current !== undefined) window.clearTimeout(collapseTimer.current);
      if (saveTimer.current !== undefined) window.clearTimeout(saveTimer.current);
    };
  }, [appWindow, flushPosition, label, persistPosition, scheduleCollapse]);

  useEffect(() => {
    if (collapsed || settingsOpen || menuOpen || dragging) return;
    collapseTimer.current = window.setTimeout(() => {
      if (pointerInsideRef.current) return;
      void Promise.all([cursorPosition(), appWindow.outerPosition(), appWindow.outerSize()]).then(([cursor, position, size]) => {
        const inside = cursor.x >= position.x && cursor.x < position.x + size.width
          && cursor.y >= position.y && cursor.y < position.y + size.height;
        if (!inside) void collapse();
      }).catch(() => void collapse());
    }, 1600);
    return () => {
      if (collapseTimer.current !== undefined) window.clearTimeout(collapseTimer.current);
    };
  }, [appWindow, collapse, collapsed, dragging, menuOpen, settingsOpen]);

  async function handleModeChange(next: WidgetMode) {
    try {
      const preferences = await setWidgetMode(next);
      setMode(preferences.mode);
      setSettingsOpen(false);
    } catch (error) {
      console.error("Não foi possível alterar o modo do widget:", error);
    }
  }

  function handlePointerDown(event: PointerEvent<HTMLDivElement>) {
    const target = event.target as HTMLElement;
    if (!target.closest("[data-tauri-drag-region]") || target.closest("button, input, textarea, select, a")) return;
    draggingRef.current = true;
    setDragging(true);
    if (collapseTimer.current !== undefined) window.clearTimeout(collapseTimer.current);
    void appWindow.startDragging().catch(() => {
      draggingRef.current = false;
      setDragging(false);
    });
  }

  function setMenuState(open: boolean) {
    menuOpenRef.current = open;
    setMenuOpen(open);
  }

  function openSettings() {
    settingsOpenRef.current = true;
    setSettingsOpen(true);
  }

  function closeSettings() {
    settingsOpenRef.current = false;
    setSettingsOpen(false);
    scheduleCollapse();
  }

  function renderPacman() {
    return <span className="pacman-scene" aria-hidden="true"><span className="pacman" /><span className="pacman-dots"><i /><i /><i /></span></span>;
  }

  return (
    <main
      className={`widget-shell ${collapsed ? "widget-shell-collapsed" : ""} ${dragging ? "is-dragging" : ""}`}
      onPointerEnter={() => { pointerInsideRef.current = true; if (collapsedRef.current) void expand(); else if (collapseTimer.current !== undefined) window.clearTimeout(collapseTimer.current); }}
      onPointerLeave={() => { pointerInsideRef.current = false; scheduleCollapse(); }}
      onPointerDown={handlePointerDown}
    >
      {collapsed ? (
        <button className={`edge-tab edge-tab-${edge}`} aria-label="Abrir painel de uso" onPointerEnter={() => void expand()} onFocus={() => void expand()}>{renderPacman()}</button>
      ) : (
        <div className={`widget-frame widget-frame-${edge} ${retracting ? "widget-frame-retracting" : ""}`}>
          {settingsOpen ? (
            <div className="settings-view">
              <header className="settings-topbar"><button onClick={closeSettings}>‹ Voltar</button><strong>Configurações</strong></header>
              <WidgetSettings usage={usage} />
            </div>
          ) : label === "compact" ? (
            <CompactUsageWidget usage={usage} mode={mode} onChangeMode={handleModeChange} onOpenSettings={openSettings} onMenuOpenChange={setMenuState} />
          ) : (
            <LargeUsageWidget usage={usage} mode={mode} onChangeMode={handleModeChange} onOpenSettings={openSettings} onMenuOpenChange={setMenuState} />
          )}
        </div>
      )}
    </main>
  );
}

export default App;
