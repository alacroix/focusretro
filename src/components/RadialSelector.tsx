import { useEffect, useState } from "react";
import { listAccounts, AccountView } from "../lib/commands";

const SIZE = 280;
const CX = SIZE / 2;
const CY = SIZE / 2;
const OUTER_R = 120;
const INNER_R = 32;
const DISC_R = OUTER_R + 10;

function wedgePath(i: number, n: number): string {
  const step = (2 * Math.PI) / n;
  const a1 = i * step - Math.PI / 2;
  const a2 = (i + 1) * step - Math.PI / 2;
  const large = step > Math.PI ? 1 : 0;
  return [
    `M ${CX + OUTER_R * Math.cos(a1)} ${CY + OUTER_R * Math.sin(a1)}`,
    `A ${OUTER_R} ${OUTER_R} 0 ${large} 1 ${CX + OUTER_R * Math.cos(a2)} ${CY + OUTER_R * Math.sin(a2)}`,
    `L ${CX + INNER_R * Math.cos(a2)} ${CY + INNER_R * Math.sin(a2)}`,
    `A ${INNER_R} ${INNER_R} 0 ${large} 0 ${CX + INNER_R * Math.cos(a1)} ${CY + INNER_R * Math.sin(a1)}`,
    "Z",
  ].join(" ");
}

function labelPos(i: number, n: number) {
  const step = (2 * Math.PI) / n;
  const mid = (i + 0.5) * step - Math.PI / 2;
  const r = (OUTER_R + INNER_R) / 2;
  return { x: CX + r * Math.cos(mid), y: CY + r * Math.sin(mid) };
}

interface Props {
  pos: { x: number; y: number } | null;
  hovered: number;
}

export default function RadialSelector({ pos, hovered }: Props) {
  const [accounts, setAccounts] = useState<AccountView[]>([]);

  useEffect(() => {
    if (pos) listAccounts().then(setAccounts);
  }, [pos]);

  const n = accounts.length;
  if (!pos || n < 2) return null;

  const accent     = "#d4721a";
  const discGrad1  = "rgba(68,56,38,0.80)";
  const discGrad2  = "rgba(30,23,14,0.94)";
  const sliceHover = "rgba(255,255,255,0.09)";
  const sliceActive= "rgba(212,114,26,0.20)";
  const sliceAH    = "rgba(212,114,26,0.34)";
  const spoke      = "rgba(210,195,165,0.12)";
  const outerRim   = "rgba(210,195,165,0.28)";
  const innerRim   = "rgba(210,195,165,0.22)";
  const shadow     = "rgba(0,0,0,0.65)";
  const textNorm   = "rgba(215,205,180,0.85)";
  const textActive = accent;
  const textHover  = "#f0e8d0";
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

  return (
    <div
      className="fixed select-none"
      style={{ left: pos.x, top: pos.y, transform: "translate(-50%,-50%)", width: SIZE, height: SIZE }}
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
            <rect width={SIZE} height={SIZE} fill="white" />
            <circle cx={CX} cy={CY} r={INNER_R} fill="black" />
          </mask>
        </defs>

        {/* Disc */}
        <circle cx={CX} cy={CY} r={DISC_R} fill="url(#disc-bg)" filter="url(#disc-shadow)" mask="url(#disc-mask)" />

        {/* Slice highlights */}
        {n === 1 ? (
          <circle cx={CX} cy={CY} r={OUTER_R} fill={sliceFill(0)} style={{ transition: "fill 0.10s" }} />
        ) : (
          accounts.map((_, i) => (
            <path key={i} d={wedgePath(i, n)} fill={sliceFill(i)} style={{ transition: "fill 0.10s" }} />
          ))
        )}

        {/* Spoke dividers */}
        {n > 1 && Array.from({ length: n }).map((_, i) => {
          const angle = (i / n) * 2 * Math.PI - Math.PI / 2;
          return (
            <line key={i}
              x1={CX + INNER_R * Math.cos(angle)} y1={CY + INNER_R * Math.sin(angle)}
              x2={CX + DISC_R  * Math.cos(angle)} y2={CY + DISC_R  * Math.sin(angle)}
              stroke={spoke} strokeWidth="1" />
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
            return <circle key={i} cx={CX} cy={CY} r={r} fill="none" stroke={accent} strokeWidth="2.5" opacity="0.75" />;
          }
          return (
            <path key={i}
              d={`M ${CX + r * Math.cos(a1)} ${CY + r * Math.sin(a1)} A ${r} ${r} 0 ${large} 1 ${CX + r * Math.cos(a2)} ${CY + r * Math.sin(a2)}`}
              fill="none" stroke={accent} strokeWidth="2.5" strokeLinecap="round" opacity="0.75"
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
        const { x, y } = labelPos(i, n);
        const isHov = i === hovered;
        const isCur = acc.is_current;
        return (
          <div key={i}
            className="absolute flex flex-col items-center gap-1 pointer-events-none"
            style={{ left: x, top: y, transform: "translate(-50%,-50%)" }}
          >
            <div
              className="w-8 h-8 rounded-full flex items-center justify-center font-bold text-xs overflow-hidden border"
              style={{
                backgroundColor: acc.icon_path ? "transparent" : (acc.color ?? "#6b7280"),
                color: "#fff",
                borderColor: isHov ? accent : (acc.color ?? iconBorder),
                boxShadow: isHov ? "0 0 0 2px rgba(212,114,26,0.30)" : "none",
                transform: isHov ? "scale(1.20) translateY(-2px)" : isCur ? "scale(1.05)" : "scale(1)",
                transition: "transform 0.15s cubic-bezier(0.34,1.56,0.64,1), border-color 0.10s, box-shadow 0.10s",
              }}
            >
              {acc.icon_path
                ? <img src={`/icons/${acc.icon_path}.png`} alt="" className="w-full h-full object-cover" />
                : <span>{acc.character_name[0]?.toUpperCase()}</span>}
            </div>

            <span style={{
              display: "block",
              fontSize: 9,
              fontWeight: isHov || isCur ? 700 : 500,
              letterSpacing: "0.05em",
              textTransform: "uppercase" as const,
              whiteSpace: "nowrap",
              maxWidth: 80,
              overflow: "hidden",
              textOverflow: "ellipsis",
              color: isHov ? textHover : isCur ? textActive : textNorm,
              textShadow,
              transform: isHov ? "translateY(2px)" : "none",
              transition: "color 0.10s, transform 0.15s cubic-bezier(0.34,1.56,0.64,1)",
            }}>
              {acc.character_name}
            </span>
          </div>
        );
      })}
    </div>
  );
}
