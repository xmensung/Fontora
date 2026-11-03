import { AnimatePresence, motion } from "motion/react";
import { Columns3, Layers, RotateCcw, Rows3, X } from "lucide-react";
import { useEffect, useRef, useState, type CSSProperties } from "react";
import { springSoft } from "../design/springs";
import { useFontCss } from "../lib/fontLoader";
import { familiesFor, useFontStore, type Family } from "../state/fontStore";
import { useFocusTrap } from "../lib/useFocusTrap";
import { FormatBadge } from "./FamilyCard";
import { GlyphText, useMissing } from "./GlyphText";
import { useT } from "../lib/i18n";

const COMPARE_SIZES = [14, 24, 40];

type CompareMode = "columns" | "rows" | "overlay";

function CompareColumn({ family, onRemove }: { family: Family; onRemove: () => void }) {
  const t = useT();
  const sampleText = useFontStore((s) => s.sampleText);
  const vertical = useFontStore((s) => s.vertical);
  const lineHeight = useFontStore((s) => s.lineHeight);
  const lead = family.faces.find((f) => f.style === "Regular") ?? family.faces[0];
  const { fontFamily } = useFontCss(lead);
  const missing = useMissing(family.name);
  const text = sampleText.trim() || family.name;

  return (
    <div className="compare-col">
      <header className="compare-col-head">
        <FormatBadge format={family.formats[0]} isVariable={family.isVariable} />
        <span className="compare-col-name">{family.name}</span>
        {missing && <span className="compare-miss">−{missing.size}</span>}
        <button className="detail-close" aria-label={t("compare.remove", { name: family.name })} onClick={onRemove}>
          <X size={13} strokeWidth={1.5} />
        </button>
      </header>
      <div
        className="compare-hero"
        style={{
          fontFamily: fontFamily ?? undefined,
          fontStyle: lead.italic ? "italic" : "normal",
        }}
      >
        Aa
      </div>
      {COMPARE_SIZES.map((px) => (
        <p
          key={px}
          className="compare-line"
          style={{
            fontFamily: fontFamily ?? undefined,
            fontSize: px,
            lineHeight,
            fontStyle: lead.italic ? "italic" : "normal",
            ...(vertical
              ? {
                  writingMode: "vertical-rl" as const,
                  textOrientation: "mixed" as const,
                  height: Math.min(px * 8, 300),
                }
              : {}),
          }}
        >
          <GlyphText text={text} missing={missing} />
        </p>
      ))}
      <div className="compare-meta tabular">
        {t("detail.stylesCount", { count: family.faces.length })} ·{" "}
        {family.formats.join(", ").toUpperCase()}
      </div>
    </div>
  );
}

function OverlayStage({ families }: { families: Family[] }) {
  const t = useT();
  const sampleText = useFontStore((s) => s.sampleText);
  const vertical = useFontStore((s) => s.vertical);
  const [opacity, setOpacity] = useState(0.62);
  const [offset, setOffset] = useState({ x: 0, y: 0 });
  const [size, setSize] = useState(56);
  const dragRef = useRef<{ x: number; y: number; ox: number; oy: number } | null>(null);

  const texts = families.map((f) => {
    const lead = f.faces.find((x) => x.style === "Regular") ?? f.faces[0];
    return { family: f, lead };
  });

  useEffect(() => {
    const onMove = (e: PointerEvent) => {
      const d = dragRef.current;
      if (!d) return;
      setOffset({ x: d.ox + (e.clientX - d.x), y: d.oy + (e.clientY - d.y) });
    };
    const onUp = () => {
      dragRef.current = null;
    };
    window.addEventListener("pointermove", onMove);
    window.addEventListener("pointerup", onUp);
    return () => {
      window.removeEventListener("pointermove", onMove);
      window.removeEventListener("pointerup", onUp);
    };
  }, []);

  const text = sampleText.trim();

  return (
    <div className="compare-stack">
      <div className="compare-stack-tools">
        <label className="compare-tool">
          <span>{t("compare.size")}</span>
          <input
            type="range"
            min={24}
            max={120}
            value={size}
            onChange={(e) => setSize(Number(e.target.value))}
          />
          <span className="tabular">{size}</span>
        </label>
        <label className="compare-tool">
          <span>{t("compare.opacity")}</span>
          <input
            type="range"
            min={20}
            max={100}
            value={Math.round(opacity * 100)}
            onChange={(e) => setOpacity(Number(e.target.value) / 100)}
          />
          <span className="tabular">{Math.round(opacity * 100)}%</span>
        </label>
        <button
          className="compare-reset"
          onClick={() => {
            setOffset({ x: 0, y: 0 });
            setOpacity(0.62);
          }}
        >
          <RotateCcw size={12} strokeWidth={1.8} />
          {t("compare.reset")}
        </button>
      </div>

      <div
        className="compare-stage"
        style={vertical ? { height: Math.min(size * 9, 460) } : undefined}
        onPointerDown={(e) => {
          dragRef.current = { x: e.clientX, y: e.clientY, ox: offset.x, oy: offset.y };
        }}
      >
        {texts.map(({ family, lead }, i) => (
          <OverlayLine
            key={family.name}
            family={family}
            lead={lead}
            text={text || family.name}
            size={size}
            opacity={opacity}
            vertical={vertical}
            color={`var(--stack-ink-${(i % 4) + 1})`}
            offset={offset}
          />
        ))}
      </div>

      <div className="compare-legend">
        {texts.map(({ family }, i) => (
          <span className="compare-legend-item" key={family.name}>
            <span
              className="compare-legend-dot"
              style={{ background: `var(--stack-ink-${(i % 4) + 1})` }}
            />
            {family.name}
          </span>
        ))}
      </div>
      <p className="compare-hint">{t("compare.overlayHint")}</p>
    </div>
  );
}

function OverlayLine({
  family,
  lead,
  text,
  size,
  opacity,
  vertical,
  color,
  offset,
}: {
  family: Family;
  lead: Family["faces"][number];
  text: string;
  size: number;
  opacity: number;
  vertical: boolean;
  color: string;
  offset: { x: number; y: number };
}) {
  const { fontFamily } = useFontCss(lead);
  return (
    <p
      className="compare-stage-line"
      style={{
        fontFamily: fontFamily ?? undefined,
        fontStyle: lead.italic ? "italic" : "normal",
        fontSize: size,
        opacity,
        color,
        transform: `translate(${offset.x}px, ${offset.y}px)`,
        ...(vertical
          ? {
              writingMode: "vertical-rl" as const,
              textOrientation: "mixed" as const,
              height: "100%",
            }
          : {}),
      }}
      aria-label={family.name}
    >
      {text}
    </p>
  );
}

export function CompareOverlay() {
  const t = useT();
  const compare = useFontStore((s) => s.compare);
  const closeCompare = useFontStore((s) => s.closeCompare);
  const openCompare = useFontStore((s) => s.openCompare);
  const fonts = useFontStore((s) => s.fonts);
  const tags = useFontStore((s) => s.tags);
  const trapRef = useFocusTrap<HTMLDivElement>(Boolean(compare));
  const [mode, setMode] = useState<CompareMode>("columns");

  useEffect(() => {
    if (!compare) return;
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && closeCompare();
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [compare, closeCompare]);

  const familyMap = familiesFor(fonts, tags);
  const families = (compare ?? [])
    .map((n) => familyMap.get(n))
    .filter((f): f is Family => Boolean(f));

  const modes: { id: CompareMode; label: string; icon: typeof Layers }[] = [
    { id: "columns", label: t("compare.modeColumns"), icon: Columns3 },
    { id: "rows", label: t("compare.modeRows"), icon: Rows3 },
    { id: "overlay", label: t("compare.modeOverlay"), icon: Layers },
  ];

  return (
    <AnimatePresence>
      {compare && families.length > 0 && (
        <motion.div
          className="compare-backdrop"
          initial={{ opacity: 0 }}
          animate={{ opacity: 1 }}
          exit={{ opacity: 0 }}
          transition={{ duration: 0.2 }}
          onClick={closeCompare}
        >
          <motion.div
            ref={trapRef}
            className="compare-panel glass-e3"
            initial={{ y: 40, scale: 0.97, opacity: 0 }}
            animate={{ y: 0, scale: 1, opacity: 1 }}
            exit={{ y: 24, scale: 0.98, opacity: 0 }}
            transition={springSoft}
            onClick={(e) => e.stopPropagation()}
            role="dialog"
            aria-label={t("compare.dialogAria")}
          >
            <header className="compare-head">
              <h2 className="compare-title">{t("compare.title")}</h2>
              <div className="compare-modes" role="tablist" aria-label={t("compare.title")}>
                {modes.map((m) => (
                  <button
                    key={m.id}
                    role="tab"
                    aria-selected={mode === m.id}
                    className={`compare-mode ${mode === m.id ? "on" : ""}`}
                    onClick={() => setMode(m.id)}
                  >
                    <m.icon size={13} strokeWidth={1.6} />
                    <span>{m.label}</span>
                  </button>
                ))}
              </div>
              <button className="detail-close" aria-label={t("compare.close")} onClick={closeCompare}>
                <X size={15} strokeWidth={1.5} />
              </button>
            </header>

            {mode === "overlay" ? (
              <OverlayStage families={families} />
            ) : (
              <div
                className={mode === "rows" ? "compare-rows" : "compare-grid"}
                style={{ "--cols": families.length } as CSSProperties}
              >
                {families.map((f) => (
                  <CompareColumn
                    key={f.name}
                    family={f}
                    onRemove={() => {
                      const next = (compare ?? []).filter((n) => n !== f.name);
                      if (next.length < 2) closeCompare();
                      else openCompare(next);
                    }}
                  />
                ))}
              </div>
            )}
          </motion.div>
        </motion.div>
      )}
    </AnimatePresence>
  );
}
