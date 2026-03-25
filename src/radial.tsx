import React, { useState } from "react";
import ReactDOM from "react-dom/client";
import { getCurrentWindow } from "@tauri-apps/api/window";
import RadialSelector from "./components/RadialSelector";
import "./index.css";

// Radial is display-only — all interaction is via Rust CGEventTap.
// Ignoring cursor events prevents cursor flicker when hovering over the overlay.
getCurrentWindow()
  .setIgnoreCursorEvents(true)
  .catch(() => {});

let _show: ((x: number, y: number) => void) | null = null;
let _hide: (() => void) | null = null;
let _hover: ((i: number) => void) | null = null;
// eslint-disable-next-line @typescript-eslint/no-explicit-any
(window as any).__radialShow = (x: number, y: number) => _show?.(x, y);
// eslint-disable-next-line @typescript-eslint/no-explicit-any
(window as any).__radialHide = () => _hide?.();
// eslint-disable-next-line @typescript-eslint/no-explicit-any
(window as any).__radialHover = (i: number) => _hover?.(i);

function RadialRoot() {
  const [pos, setPos] = useState<{ x: number; y: number } | null>(null);
  const [hovered, setHovered] = useState(-1);

  // eslint-disable-next-line react-hooks/globals
  _show = (x, y) => {
    setHovered(-1);
    setPos({ x, y });
  };
  // eslint-disable-next-line react-hooks/globals
  _hide = () => {
    setPos(null);
    setHovered(-1);
  };
  // eslint-disable-next-line react-hooks/globals
  _hover = setHovered;

  return <RadialSelector pos={pos} hovered={hovered} />;
}

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <RadialRoot />
  </React.StrictMode>,
);
