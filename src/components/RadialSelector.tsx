import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";

import { listAccounts, AccountView } from "../lib/commands";

const INNER_R = 32;

function labelFontSize(name: string): number {
  if (name.length > 14) return 7;
  if (name.length > 10) return 8;
  return 9;
}

function wedgePath(
  i: number,
  n: number,
  cx: number,
  cy: number,
  outerR: number,
  innerR: number,
): string {
  const step = (2 * Math.PI) / n;
  const a1 = i * step - Math.PI / 2;
  const a2 = (i + 1) * step - Math.PI / 2;
  const large = step > Math.PI ? 1 : 0;
  return [
    `M ${cx + outerR * Math.cos(a1)} ${cy + outerR * Math.sin(a1)}`,
    `A ${outerR} ${outerR} 0 ${large} 1 ${cx + outerR * Math.cos(a2)} ${cy + outerR * Math.sin(a2)}`,
    `L ${cx + innerR * Math.cos(a2)} ${cy + innerR * Math.sin(a2)}`,
    `A ${innerR} ${innerR} 0 ${large} 0 ${cx + innerR * Math.cos(a1)} ${cy + innerR * Math.sin(a1)}`,
    "Z",
  ].join(" ");
}

function labelPos(i: number, n: number, cx: number, cy: number, outerR: number, innerR: number) {
  const step = (2 * Math.PI) / n;
  const mid = (i + 0.5) * step - Math.PI / 2;
  const r = (outerR + innerR) / 2;
  return { x: cx + r * Math.cos(mid), y: cy + r * Math.sin(mid) };
}

interface Props {
  pos: { x: number; y: number } | null;
  hovered: number;
  accounts?: AccountView[];
}

export default function RadialSelector({ pos, hovered, accounts: accountsProp }: Props) {
  const { t } = useTranslation();
  const [accounts, setAccounts] = useState<AccountView[]>(accountsProp ?? []);

  useEffect(() => {
    if (accountsProp) return;
    if (pos) listAccounts().then(setAccounts);
  }, [pos, accountsProp]);

  const n = accounts.length;
  if (!pos || n < 2) return null;

  const isLarge = n >= 5;
  const SIZE = isLarge ? 350 : 280;
  const OUTER_R = isLarge ? 150 : 120;
  const DISC_R = OUTER_R + 10;
  const CX = SIZE / 2;
  const CY = SIZE / 2;
  const MASK_OFFSET = isLarge ? 75 : 60;

  const accent = "#d4721a";
  const discGrad1 = "rgba(68,56,38,0.80)";
  const discGrad2 = "rgba(30,23,14,0.94)";
  const sliceHover = "rgba(255,255,255,0.09)";
  const sliceActive = "rgba(212,114,26,0.20)";
  const sliceAH = "rgba(212,114,26,0.34)";
  const spoke = "rgba(210,195,165,0.12)";
  const outerRim = "rgba(210,195,165,0.28)";
  const innerRim = "rgba(210,195,165,0.22)";
  const shadow = "rgba(0,0,0,0.65)";
  const textNorm = "rgba(215,205,180,0.85)";
  const textActive = accent;
  const textHover = "#f0e8d0";
  const textShadow = "0 1px 4px rgba(0,0,0,0.90)";
  const iconBorder = "rgba(210,195,165,0.22)";

  const sliceFill = (i: number) => {
    const isHov = i === hovered;
    const isCur = accounts[i].is_current;
    if (isHov && isCur) return sliceAH;
    if (isHov) return sliceHover;
    if (isCur) return sliceActive;
    return "none";
  };

  const positioning = accountsProp ? "absolute" : "fixed";

  return (
    <div
      className="select-none"
      style={{
        position: positioning,
        left: pos.x,
        top: pos.y,
        transform: "translate(-50%,-50%)",
        width: SIZE,
        height: SIZE,
      }}
    >
      <svg width={SIZE} height={SIZE} overflow="visible" className="absolute inset-0">
        <defs>
          <radialGradient id="disc-bg" cx="40%" cy="35%" r="65%">
            <stop offset="0%" stopColor={discGrad1} />
            <stop offset="100%" stopColor={discGrad2} />
          </radialGradient>
          <filter id="disc-shadow" x="-30%" y="-30%" width="160%" height="160%">
            <feDropShadow dx="0" dy="4" stdDeviation="12" floodColor={shadow} floodOpacity="1" />
          </filter>
          <mask id="disc-mask">
            <rect
              x={-MASK_OFFSET}
              y={-MASK_OFFSET}
              width={SIZE + MASK_OFFSET * 2}
              height={SIZE + MASK_OFFSET * 2}
              fill="white"
            />
            <circle cx={CX} cy={CY} r={INNER_R} fill="black" />
          </mask>
        </defs>

        {/* Disc */}
        <circle
          cx={CX}
          cy={CY}
          r={DISC_R}
          fill="url(#disc-bg)"
          filter="url(#disc-shadow)"
          mask="url(#disc-mask)"
        />

        {/* Slice highlights */}
        {n === 1 ? (
          <circle
            cx={CX}
            cy={CY}
            r={OUTER_R}
            fill={sliceFill(0)}
            style={{ transition: "fill 0.10s" }}
          />
        ) : (
          accounts.map((_, i) => (
            <path
              key={i}
              d={wedgePath(i, n, CX, CY, OUTER_R, INNER_R)}
              fill={sliceFill(i)}
              style={{ transition: "fill 0.10s" }}
            />
          ))
        )}

        {/* Spoke dividers */}
        {n > 1 &&
          Array.from({ length: n }).map((_, i) => {
            const angle = (i / n) * 2 * Math.PI - Math.PI / 2;
            return (
              <line
                key={i}
                x1={CX + INNER_R * Math.cos(angle)}
                y1={CY + INNER_R * Math.sin(angle)}
                x2={CX + DISC_R * Math.cos(angle)}
                y2={CY + DISC_R * Math.sin(angle)}
                stroke={spoke}
                strokeWidth="1"
              />
            );
          })}

        {/* Active account: brand arc on outer ring */}
        {accounts.map((acc, i) => {
          if (!acc.is_current) return null;
          const step = (2 * Math.PI) / n;
          const a1 = i * step - Math.PI / 2 + 0.04;
          const a2 = (i + 1) * step - Math.PI / 2 - 0.04;
          const r = DISC_R - 1;
          const large = a2 - a1 > Math.PI ? 1 : 0;
          if (n === 1) {
            return (
              <circle
                key={i}
                cx={CX}
                cy={CY}
                r={r}
                fill="none"
                stroke={accent}
                strokeWidth="2.5"
                opacity="0.75"
              />
            );
          }
          return (
            <path
              key={i}
              d={`M ${CX + r * Math.cos(a1)} ${CY + r * Math.sin(a1)} A ${r} ${r} 0 ${large} 1 ${CX + r * Math.cos(a2)} ${CY + r * Math.sin(a2)}`}
              fill="none"
              stroke={accent}
              strokeWidth="2.5"
              strokeLinecap="round"
              opacity="0.75"
            />
          );
        })}

        {/* Outer rim */}
        <circle cx={CX} cy={CY} r={DISC_R} fill="none" stroke={outerRim} strokeWidth="1" />
        {/* Inner rim */}
        <circle cx={CX} cy={CY} r={INNER_R} fill="none" stroke={innerRim} strokeWidth="1.5" />
      </svg>

      {/* Labels */}
      {accounts.map((acc, i) => {
        const { x, y } = labelPos(i, n, CX, CY, OUTER_R, INNER_R);
        const isHov = i === hovered;
        const isCur = acc.is_current;
        const isUpper = y < CY - 40;
        const isMiddle = Math.abs(y - CY) < 40;
        const xShift = n >= 7 ? (x < CX - 5 ? -15 : x > CX + 5 ? 15 : 0) : 0;
        return (
          <div
            key={i}
            className={`pointer-events-none absolute flex items-center gap-2 ${isUpper ? "flex-col-reverse" : "flex-col"}`}
            style={{
              left: x,
              top: y,
              transform: `translate(calc(-50% + ${isMiddle ? xShift : 0}px),-50%)`,
            }}
          >
            <div
              className="flex h-8 w-8 items-center justify-center overflow-hidden rounded-full border text-xs font-bold"
              style={{
                backgroundColor: acc.icon_path ? "transparent" : (acc.color ?? "#6b7280"),
                color: "#fff",
                borderColor: isHov ? accent : (acc.color ?? iconBorder),
                boxShadow: isHov ? "0 0 0 2px rgba(212,114,26,0.30)" : "none",
                transform: isHov
                  ? "scale(1.20) translateY(-2px)"
                  : isCur
                    ? "scale(1.05)"
                    : "scale(1)",
                transition:
                  "transform 0.15s cubic-bezier(0.34,1.56,0.64,1), border-color 0.10s, box-shadow 0.10s",
              }}
            >
              {acc.is_connection_state ? (
                <svg
                  width="14"
                  height="14"
                  viewBox="0 0 24 24"
                  fill="none"
                  stroke="currentColor"
                  strokeWidth="2"
                  strokeLinecap="round"
                  strokeLinejoin="round"
                >
                  <path d="M5 22h14" />
                  <path d="M5 2h14" />
                  <path d="M17 22v-4.172a2 2 0 0 0-.586-1.414L12 12l-4.414 4.414A2 2 0 0 0 7 17.828V22" />
                  <path d="M7 2v4.172a2 2 0 0 0 .586 1.414L12 12l4.414-4.414A2 2 0 0 0 17 6.172V2" />
                </svg>
              ) : acc.icon_path ? (
                <img
                  src={`/icons/${acc.icon_path}.png`}
                  alt=""
                  className="h-full w-full object-cover"
                />
              ) : (
                <span>{acc.character_name[0]?.toUpperCase()}</span>
              )}
            </div>

            <span
              style={{
                display: "block",
                fontSize: labelFontSize(acc.character_name),
                fontWeight: isHov || isCur ? 700 : 500,
                letterSpacing: "0.05em",
                textTransform: "uppercase" as const,
                whiteSpace: "nowrap",
                maxWidth: 80,
                overflow: "hidden",
                textOverflow: "ellipsis",
                color: isHov ? textHover : isCur ? textActive : textNorm,
                textShadow,
                transform: isHov
                  ? `translateX(${isMiddle ? 0 : xShift}px) translateY(2px)`
                  : !isMiddle && xShift
                    ? `translateX(${xShift}px)`
                    : "none",
                transition: "color 0.10s, transform 0.15s cubic-bezier(0.34,1.56,0.64,1)",
              }}
            >
              {acc.is_connection_state
                ? (() => {
                    const conn = accounts.filter((a) => a.is_connection_state);
                    const ci = conn.findIndex((a) => a.window_id === acc.window_id);
                    return conn.length > 1
                      ? t("accounts.connecting_n", { n: ci + 1 })
                      : t("accounts.connecting");
                  })()
                : acc.character_name}
            </span>
          </div>
        );
      })}
    </div>
  );
}
