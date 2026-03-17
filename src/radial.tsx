import React, { useState } from "react";
import ReactDOM from "react-dom/client";
import { getCurrentWindow } from "@tauri-apps/api/window";
import RadialSelector from "./components/RadialSelector";
import "./index.css";

// Radial is display-only — all interaction is via Rust CGEventTap.
// Ignoring cursor events prevents cursor flicker when hovering over the overlay.
getCurrentWindow().setIgnoreCursorEvents(true).catch(() => {});

let _show: ((x: number, y: number, theme: string) => void) | null = null;
let _hide: (() => void) | null = null;
let _hover: ((i: number) => void) | null = null;
(window as any).__radialShow = (x: number, y: number, theme: string) => _show?.(x, y, theme);
(window as any).__radialHide = () => _hide?.();
(window as any).__radialHover = (i: number) => _hover?.(i);

function applyTheme(theme: string) {
  const dark = theme === "dark" || (theme === "system" && window.matchMedia("(prefers-color-scheme: dark)").matches);
  document.documentElement.classList.toggle("dark", dark);
}

function RadialRoot() {
  const [pos, setPos] = useState<{ x: number; y: number } | null>(null);
  const [hovered, setHovered] = useState(-1);
  const [isDark, setIsDark] = useState(false);

  _show = (x, y, theme) => {
    const dark = theme === "dark" || (theme === "system" && window.matchMedia("(prefers-color-scheme: dark)").matches);
    applyTheme(theme);
    setIsDark(dark);
    setHovered(-1);
    setPos({ x, y });
  };
  _hide = () => { setPos(null); setHovered(-1); };
  _hover = setHovered;

  return <RadialSelector pos={pos} hovered={hovered} isDark={isDark} />;
}

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode><RadialRoot /></React.StrictMode>
);
