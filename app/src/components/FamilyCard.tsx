import { AnimatePresence, motion } from "motion/react";
import { AlertTriangle, ChevronRight, Lock, Star } from "lucide-react";
import { memo, useMemo, useState } from "react";
import { PillToggle } from "../design/primitives/PillToggle";
import { spring, springSnappy, springSoft, staggerDelay } from "../design/springs";
import { useBlankGaps, useFontCss, useInkOk } from "../lib/fontLoader";
import { buildFamilyMenu } from "../lib/menus";
import { openContextMenu } from "../design/primitives/ContextMenu";
import { activeConflictsFor, conflictsFor, SIZES, useFontStore, type Family } from "../state/fontStore";
import { playStar } from "../lib/sound";
import type { FontFace } from "../lib/ipc";
import { useT } from "../lib/i18n";
import { GlyphText, useMissing } from "./GlyphText";
import { missingLabel } from "../lib/glyphs";

export function FormatBadge({ format, isVariable }: { format: string; isVariable: boolean }) {
  if (isVariable) return <span className="badge badge-variable">VAR</span>;
  return <span className={`badge badge-${format}`}>{format.toUpperCase()}</span>;
}

function FacePreview({
  face,
  text,
  fallback,
  size,
  index,
  synthesize,
}: {
  face: FontFace;
  text: string;
  fallback: string;
  size: number;
  index: number;
  synthesize: boolean;
}) {
  const t = useT();
  const { fontFamily, failed } = useFontCss(face);
  const inkOk = useInkOk(fontFamily, text);
  const toName = Boolean(fontFamily) && !inkOk;
  const nameInk = useInkOk(fontFamily, toName ? fallback : "");
  const shown = toName ? (nameInk ? fallback : null) : text;
  return (
    <motion.div
      className="face-row"
      initial={{ opacity: 0, y: 8 }}
      animate={{ opacity: 1, y: 0 }}
      transition={{ ...spring, delay: staggerDelay(index) }}
    >
      <span className="face-style">{face.style}</span>
      <AnimatePresence mode="wait">
        {fontFamily && shown ? (
          <motion.span
            key="real"
            className="face-sample"
            style={{
              fontFamily,
              fontSize: Math.min(size, 32),
              fontStyle: face.italic ? "italic" : "normal",

              fontWeight: synthesize ? face.weight : undefined,
            }}
            initial={{ opacity: 0, y: 4 }}
            animate={{ opacity: 1, y: 0 }}
            transition={spring}
          >
            {shown}
          </motion.span>
        ) : fontFamily ? (
          <span key="blank" className="face-sample face-failed">
            {t("card.blankGlyphs")}
          </span>
        ) : failed ? (
          <span key="failed" className="face-sample face-failed">
            {t("card.previewUnavailable")}
          </span>
        ) : (
          <motion.span
            key="skeleton"
            className="skeleton face-skeleton"
            exit={{ opacity: 0, transition: { duration: 0.1 } }}
          />
        )}
      </AnimatePresence>
    </motion.div>
  );
}

export const FamilyCard = memo(function FamilyCard({ family }: { family: Family }) {
  const t = useT();
  const sampleText = useFontStore((s) => s.sampleText);
  const sizeIndex = useFontStore((s) => s.sizeIndex);
  const setFamilyActive = useFontStore((s) => s.setFamilyActive);
  const selectWith = useFontStore((s) => s.selectWith);
  const selected = useFontStore((s) => s.selection.includes(family.name));
  const favorite = useFontStore((s) => s.favorites.includes(family.name));
  const rating = useFontStore((s) => s.ratings[family.name] ?? 0);
  const toggleFavorite = useFontStore((s) => s.toggleFavorite);
  const hasActiveConflict = useFontStore((s) => activeConflictsFor(s.fonts).has(family.name));
  const hasDormantConflict = useFontStore((s) => conflictsFor(s.fonts).has(family.name));
  const [expanded, setExpanded] = useState(false);
  const vertical = useFontStore((s) => s.vertical);
  const lineHeight = useFontStore((s) => s.lineHeight);
  const missing = useMissing(family.name);

  const size = SIZES[sizeIndex];
  const lead = family.faces.find((f) => f.style === "Regular") ?? family.faces[0];
  const { fontFamily, failed } = useFontCss(lead);
  const text = sampleText.trim() || family.name;
  // 有些字体把英文码位挂成空字形（日文毛笔一类），照原样画就是一整块白。
  // 先实画一遍，没有墨就改用字体名 —— 名字里的字它们基本都有。
  const inkOk = useInkOk(fontFamily, text);

  // 整句有墨不代表每个字都有墨：日文毛笔体常常一半正常、一半空字形，
  // 照原样画就是「粗细极不均匀」。逐字挑出空的那几个，一并算进缺字。
  // 只在已知有缺字时才扫 —— 正常字体（覆盖率完整）没必要为一个字跑一遍画布。
  const shouldScan = Boolean(missing && missing.size > 0);
  const gaps = useBlankGaps(fontFamily, fontFamily && shouldScan ? text : "");
  const missingAll = useMemo(() => {
    if (!gaps.size) return missing;
    const set = new Set(missing ?? []);
    for (const cp of gaps) set.add(cp);
    return set;
  }, [missing, gaps]);

  // 缺字太多就别硬排版了 —— 一行里大半是补的，读起来只剩噪点。
  // 四成以上的字没有字形（白舟那种日文毛笔体常到七成），直接拿字体名当样张。
  const uniqueChars = useMemo(() => new Set(Array.from(text)).size, [text]);
  const missRatio = missingAll ? missingAll.size / Math.max(1, uniqueChars) : 0;
  const toName = Boolean(fontFamily) && (!inkOk || missRatio >= 0.4);
  const nameInk = useInkOk(fontFamily, toName ? family.name : "");
  const shown = toName ? (nameInk ? family.name : null) : text;

  return (
    <motion.article
      className={`family-card ${selected ? "card-selected" : ""}`}
      data-family={family.name}
      tabIndex={0}
      role="button"
      aria-label={family.name}
      aria-pressed={selected}
      onClick={(e) => {
        const st = useFontStore.getState();

        if (st.comparePicking) {
          st.togglePick(family.name);
          return;
        }
        const mode = e.ctrlKey || e.metaKey ? "toggle" : e.shiftKey ? "range" : "single";
        selectWith(family.name, mode, st.visibleOrder);
        // 单击（不带 Ctrl/Shift）＝直接进整页详情，不再有右侧面板这一步。
        if (mode === "single") st.openFontDetail(family.name);
      }}
      onKeyDown={(e) => {
        if (e.key !== "Enter" || e.target !== e.currentTarget) return;
        e.preventDefault();
        const st = useFontStore.getState();
        selectWith(family.name, "single", st.visibleOrder);
        st.openFontDetail(family.name);
      }}
      onContextMenu={(e) => openContextMenu(e, buildFamilyMenu(family))}
    >
      <header className="card-head">
        <FormatBadge format={family.formats[0]} isVariable={family.isVariable} />
        <span className="card-name">{family.name}</span>
        {family.faces.length > 1 && (
          <motion.button
            className="card-expand"
            aria-label={t(expanded ? "card.collapseStyles" : "card.expandStyles")}
            aria-expanded={expanded}
            onClick={(e) => {
              e.stopPropagation();
              setExpanded((v) => !v);
            }}
            whileHover={{ y: -1 }}
            whileTap={{ scale: 0.94 }}
          >
            <motion.span
              className="chevron"
              animate={{ rotate: expanded ? 90 : 0 }}
              transition={springSnappy}
            >
              <ChevronRight size={13} strokeWidth={1.5} />
            </motion.span>
            <span className="tabular">
              {t("card.stylesCount", { count: family.faces.length })}
            </span>
          </motion.button>
        )}
        <span className="card-spacer" />
        {hasActiveConflict ? (
          <span
            className="conflict-badge"
            title=""
            aria-label={t("card.conflict")}
          >
            <AlertTriangle size={13} strokeWidth={1.5} />
          </span>
        ) : (
          hasDormantConflict && (
            <span
              className="conflict-badge conflict-badge-dormant"
              title=""
              aria-label={t("card.conflictResolved")}
            >
              <AlertTriangle size={13} strokeWidth={1.5} />
            </span>
          )
        )}
        <motion.button
          className={`star-btn ${favorite ? "star-on" : ""}`}
          aria-label={t(favorite ? "card.unfavorite" : "card.favorite", { name: family.name })}
          aria-pressed={favorite}
          onClick={(e) => {
            e.stopPropagation();
            playStar(!favorite);
            void toggleFavorite(family.name);
          }}
          whileTap={{ scale: 0.85 }}
        >
          <Star size={14} strokeWidth={1.5} />
        </motion.button>
        {family.deactivatable ? (
          <PillToggle
            on={family.active}
            onChange={(on) => void setFamilyActive(family.name, on)}
            label={t(family.active ? "card.deactivate" : "card.activate", { name: family.name })}
          />
        ) : (
          <span className="card-os-lock" title={t("card.osManaged")} aria-label={t("card.osManaged")}>
            <Lock size={11} strokeWidth={1.75} />
            {t("card.osManagedShort")}
          </span>
        )}
      </header>

      {rating > 0 && (
        <span
          className="card-rate"
          title={t("rating.value", { value: rating })}
          aria-label={t("rating.value", { value: rating })}
        >
          <Star size={9} strokeWidth={2.4} fill="currentColor" />
          <span className="tabular">{rating}</span>
        </span>
      )}

      <div
        className={`card-preview ${vertical ? "preview-vertical" : ""}`}
        style={{
          minHeight: vertical ? undefined : size * 1.2,
          height: vertical ? Math.min(size * 3.4, 250) : undefined,
        }}
      >
        <AnimatePresence mode="wait">
          {fontFamily && shown ? (
            <motion.p
              key="real"
              className="preview-text"
              style={{
                fontFamily,
                fontSize: size,
                lineHeight,
                fontStyle: lead.italic ? "italic" : "normal",
                ...(vertical
                  ? {
                      writingMode: "vertical-rl" as const,
                      textOrientation: "mixed" as const,
                      height: "100%",
                    }
                  : {}),
              }}
              initial={{ opacity: 0, y: 5 }}
              animate={{ opacity: 1, y: 0 }}
              transition={spring}
            >
              {toName ? shown : <GlyphText text={text} missing={missingAll} />}
            </motion.p>
          ) : fontFamily ? (
            <p key="blank" className="preview-text preview-failed">
              {t("card.blankGlyphs")}
            </p>
          ) : failed ? (
            <p key="failed" className="preview-text preview-failed">
              {t("card.noPreview")}
            </p>
          ) : (
            <motion.span
              key="skeleton"
              className="skeleton preview-skeleton"
              style={{ height: size * 1.1 }}
              exit={{ opacity: 0, transition: { duration: 0.1 } }}
            />
          )}
        </AnimatePresence>
      </div>

      {toName && (
        <span className="card-missing card-hint" title={t("card.namePreviewTitle")}>
          {t("card.namePreview")}
          {missingAll && missingAll.size > 0 && (
            <span className="tabular"> · {t("card.missing", { count: missingAll.size })}</span>
          )}
        </span>
      )}

      {!toName && missingAll && missingAll.size > 0 && (
        <span
          className="card-missing"
          title={t("card.missingTitle", { chars: missingLabel(missingAll) })}
        >
          {t("card.missing", { count: missingAll.size })}
        </span>
      )}

      <AnimatePresence initial={false}>
        {expanded && (
          <motion.div
            className="card-styles"
            initial={{ height: 0, opacity: 0 }}
            animate={{ height: "auto", opacity: 1 }}
            exit={{ height: 0, opacity: 0, transition: { ...springSnappy } }}
            transition={springSoft}
          >
            {family.faces.map((face, i) => (
              <FacePreview
                key={face.id}
                face={face}
                text={text}
                fallback={family.name}
                size={size}
                index={i}
                synthesize={family.faces.filter((f) => f.path === face.path).length > 1}
              />
            ))}
          </motion.div>
        )}
      </AnimatePresence>
    </motion.article>
  );
});
