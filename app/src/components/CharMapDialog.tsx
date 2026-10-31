import { AnimatePresence, motion } from "motion/react";
import { Check, Copy, Grid3x3, LoaderCircle, Search, X } from "lucide-react";
import { useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import { springSnappy } from "../design/springs";
import { toast } from "../design/primitives/Toast";
import { ipc, type CharMap } from "../lib/ipc";
import { useFocusTrap } from "../lib/useFocusTrap";
import { useT } from "../lib/i18n";

interface Props {
  open: boolean;
  onClose: () => void;
  path: string;
  faceIndex: number;
  family: string;
  /** 用这款字体自己的 CSS 家族来渲染格子，看到的就是真实字形。 */
  fontFamily: string | null;
}

/** 格子最小边长；列数按容器宽度算，保证一屏能看全一行。 */
const CELL_MIN = 38;
const ROW_GAP = 2;
/** 对应 `.cm-grid` 的左右内边距 14px × 2 —— 列宽必须扣掉它，否则最后几列会被裁掉。 */
const GRID_PAD_X = 28;

function fmt(cp: number) {
  return `U+${cp.toString(16).toUpperCase().padStart(4, "0")}`;
}

function inRanges(ranges: [number, number][], cp: number) {
  let lo = 0;
  let hi = ranges.length;
  while (lo < hi) {
    const mid = (lo + hi) >> 1;
    const [start, end] = ranges[mid];
    if (cp < start) hi = mid;
    else if (cp > end) lo = mid + 1;
    else return true;
  }
  return false;
}

/** 「这个字到底有没有」——需求里最实在的一条：输入一个字，立刻给结论。 */
export function CharMapDialog({ open: isOpen, onClose, path, faceIndex, family, fontFamily }: Props) {
  const t = useT();
  const trapRef = useFocusTrap<HTMLDivElement>(isOpen);
  const [map, setMap] = useState<CharMap | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [activeId, setActiveId] = useState<string | null>(null);
  const [query, setQuery] = useState("");
  const [picked, setPicked] = useState<number | null>(null);
  const [scrollTop, setScrollTop] = useState(0);
  const [viewport, setViewport] = useState({ w: 0, h: 0 });
  const gridRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!isOpen) return;
    let stale = false;
    setMap(null);
    setError(null);
    setQuery("");
    setPicked(null);
    setScrollTop(0);
    ipc
      .fontCharmap(path, faceIndex)
      .then((m) => {
        if (stale) return;
        setMap(m);
        // 中文字体十有八九是来看汉字的，默认落在汉字区。
        const first =
          m.blocks.find((b) => b.id === "cjk") ??
          m.blocks.find((b) => b.id === "pua") ??
          m.blocks[0];
        setActiveId(first?.id ?? null);
      })
      .catch((e) => {
        if (!stale) setError(String(e));
      });
    return () => {
      stale = true;
    };
  }, [isOpen, path, faceIndex]);

  useEffect(() => {
    if (!isOpen) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        e.stopPropagation();
        onClose();
      }
    };
    window.addEventListener("keydown", onKey, true);
    return () => window.removeEventListener("keydown", onKey, true);
  }, [isOpen, onClose]);

  const active = useMemo(
    () => map?.blocks.find((b) => b.id === activeId) ?? null,
    [map, activeId],
  );

  // 当前区段的「有没有」表：一次算好，滚动时直接下标取值。
  const covered = useMemo(() => {
    if (!map || !active) return null;
    const span = active.end - active.start + 1;
    const flags = new Uint8Array(span);
    for (let i = 0; i < span; i += 1) {
      flags[i] = inRanges(map.ranges, active.start + i) ? 1 : 0;
    }
    return flags;
  }, [map, active]);

  useLayoutEffect(() => {
    const el = gridRef.current;
    if (!el) return;
    const read = () => setViewport({ w: el.clientWidth, h: el.clientHeight });
    read();
    const ro = new ResizeObserver(read);
    ro.observe(el);
    return () => ro.disconnect();
  }, [isOpen, map, activeId]);

  // 列数与格子边长都要把 gap 算进去，否则累计误差会让最后一行错位、或者多出半个格子。
  const usable = Math.max(0, viewport.w - GRID_PAD_X);
  const cols = Math.max(6, Math.floor((usable + ROW_GAP) / (CELL_MIN + ROW_GAP)) || 6);
  const cell = usable > 0 ? (usable - ROW_GAP * (cols - 1)) / cols : CELL_MIN;
  const rowH = cell + ROW_GAP;
  const rows = active ? Math.ceil((active.end - active.start + 1) / cols) : 0;
  const from = Math.max(0, Math.floor(scrollTop / rowH) - 3);
  const to = Math.min(rows, Math.ceil((scrollTop + (viewport.h || 800)) / rowH) + 3);

  const scrollToCode = (cp: number, blockId: string) => {
    const block = map?.blocks.find((b) => b.id === blockId);
    if (!block) return;
    const index = cp - block.start;
    const row = Math.floor(index / cols);
    const el = gridRef.current;
    if (el) el.scrollTop = Math.max(0, row * rowH - (viewport.h || 600) / 2);
  };

  const search = useMemo(() => {
    const text = query.trim();
    if (!text || !map) return null;
    const cp = text.codePointAt(0);
    if (cp === undefined) return null;
    const ch = String.fromCodePoint(cp);
    const supported = inRanges(map.ranges, cp);
    const block = map.blocks.find((b) => cp >= b.start && cp <= b.end) ?? null;
    const name = map.glyphNames.find((g) => g.cp === cp)?.name ?? null;
    return { ch, cp, supported, block, name };
  }, [query, map]);

  const jumpTo = (cp: number) => {
    if (!map) return;
    const block = map.blocks.find((b) => cp >= b.start && cp <= b.end);
    if (!block) return;
    setActiveId(block.id);
    setPicked(cp);
    // 等区段切换渲染完成再滚，否则还在旧区段里算行号。
    requestAnimationFrame(() => {
      requestAnimationFrame(() => scrollToCode(cp, block.id));
    });
  };

  const copy = (text: string) => {
    void navigator.clipboard.writeText(text).then(
      () => toast.success(t("fontDetail.copied"), text),
      () => toast.error(t("fontDetail.copyFailed"), text),
    );
  };

  const pickedBlock = useMemo(() => {
    if (!map || picked === null) return null;
    return map.blocks.find((b) => picked >= b.start && picked <= b.end) ?? null;
  }, [map, picked]);
  const pickedName = useMemo(
    () => (picked === null ? null : map?.glyphNames.find((g) => g.cp === picked)?.name ?? null),
    [map, picked],
  );

  return (
    <AnimatePresence>
      {isOpen && (
        <motion.div
          className="cm-backdrop"
          initial={{ opacity: 0 }}
          animate={{ opacity: 1 }}
          exit={{ opacity: 0, transition: { duration: 0.12 } }}
          onClick={onClose}
        >
          <motion.div
            className="cm-panel"
            ref={trapRef}
            role="dialog"
            aria-modal="true"
            aria-label={t("cmap.title")}
            initial={{ opacity: 0, scale: 0.985, y: 10 }}
            animate={{ opacity: 1, scale: 1, y: 0 }}
            exit={{ opacity: 0, scale: 0.99, y: 6, transition: { duration: 0.12 } }}
            transition={springSnappy}
            onClick={(e) => e.stopPropagation()}
          >
            <header className="cm-head">
              <span className="cm-head-icon" aria-hidden="true">
                <Grid3x3 size={15} strokeWidth={1.7} />
              </span>
              <div className="cm-head-text">
                <div className="cm-title">{t("cmap.title")}</div>
                <div className="cm-sub">
                  {family}
                  {map ? ` · ${t("cmap.total", { count: map.total.toLocaleString() })}` : ""}
                  {map?.hasLigatures ? ` · ${t("cmap.ligatures")}` : ""}
                </div>
              </div>
              <button className="cm-close" aria-label={t("export.close")} onClick={onClose}>
                <X size={15} strokeWidth={1.7} />
              </button>
            </header>

            <div className="cm-toolbar">
              <label className="cm-search">
                <Search size={13} strokeWidth={1.7} />
                <input
                  value={query}
                  onChange={(e) => setQuery(e.target.value)}
                  placeholder={t("cmap.searchPlaceholder")}
                  spellCheck={false}
                />
              </label>
              {search && (
                <div className={`cm-verdict ${search.supported ? "ok" : "no"}`}>
                  {search.supported ? <Check size={13} strokeWidth={2.2} /> : <X size={13} strokeWidth={2.2} />}
                  <span className="cm-verdict-char" style={fontFamily ? { fontFamily } : undefined}>
                    {search.ch}
                  </span>
                  <span>{search.supported ? t("cmap.supported") : t("cmap.unsupported")}</span>
                  <span className="cm-verdict-code tabular">{fmt(search.cp)}</span>
                  {search.name && <span className="cm-verdict-name">{search.name}</span>}
                  {search.supported && search.block && (
                    <button className="cm-jump" onClick={() => jumpTo(search.cp)}>
                      {t("cmap.jump")}
                    </button>
                  )}
                </div>
              )}
            </div>

            {error && (
              <div className="cm-error">
                <strong>{t("cmap.failed")}</strong>
                <span className="fd-mono">{error}</span>
              </div>
            )}

            {!error && !map && (
              <div className="cm-loading">
                <LoaderCircle size={16} strokeWidth={1.6} className="fd-spin" />
                {t("fontDetail.loading")}
              </div>
            )}

            {map && map.blocks.length === 0 && (
              <div className="cm-loading">{t("cmap.empty")}</div>
            )}

            {map && map.blocks.length > 0 && (
              <div className="cm-body">
                <aside className="cm-blocks">
                  {map.blocks.map((b) => (
                    <button
                      key={b.id}
                      className={`cm-block ${b.id === activeId ? "on" : ""}`}
                      onClick={() => {
                        setActiveId(b.id);
                        setScrollTop(0);
                      }}
                    >
                      <span className="cm-block-label" title={b.label}>
                        {b.label}
                      </span>
                      <span className="cm-block-count tabular">{b.covered.toLocaleString()}</span>
                    </button>
                  ))}
                  {map.outside > 0 && (
                    <p className="cm-outside">
                      {t("cmap.outside", { count: map.outside.toLocaleString() })}
                    </p>
                  )}
                </aside>

                <div className="cm-main">
                  {active && (
                    <div className="cm-main-head">
                      <span className="cm-main-title">{active.label}</span>
                      <span className="cm-main-range tabular">
                        {fmt(active.start)} – {fmt(active.end)}
                      </span>
                      <span className="cm-main-count tabular">
                        {t("cmap.coveredOf", {
                          covered: active.covered.toLocaleString(),
                          total: (active.end - active.start + 1).toLocaleString(),
                        })}
                      </span>
                    </div>
                  )}
                  <div
                    className="cm-grid"
                    ref={gridRef}
                    onScroll={(e) => setScrollTop(e.currentTarget.scrollTop)}
                  >
                    <div className="cm-grid-space" style={{ height: rows * rowH }}>
                      {Array.from({ length: Math.max(0, to - from) }, (_, i) => {
                        const row = from + i;
                        const cells = [];
                        for (let c = 0; c < cols; c += 1) {
                          const index = row * cols + c;
                          if (!active || index >= active.end - active.start + 1) {
                            cells.push(<span key={c} className="cm-cell blank" />);
                            continue;
                          }
                          const cp = active.start + index;
                          const has = covered?.[index] === 1;
                          cells.push(
                            <button
                              key={c}
                              className={`cm-cell ${has ? "on" : "off"} ${picked === cp ? "picked" : ""}`}
                              style={{ fontFamily: fontFamily ?? undefined }}
                              disabled={!has}
                              title={has ? `${String.fromCodePoint(cp)} ${fmt(cp)}` : fmt(cp)}
                              onClick={() => has && setPicked(cp)}
                            >
                              {has ? String.fromCodePoint(cp) : ""}
                            </button>,
                          );
                        }
                        return (
                          <div
                            key={row}
                            className="cm-row"
                            style={{ top: row * rowH, height: cell, gridTemplateColumns: `repeat(${cols}, 1fr)` }}
                          >
                            {cells}
                          </div>
                        );
                      })}
                    </div>
                  </div>

                  <div className="cm-detail">
                    {picked === null ? (
                      <span className="cm-detail-hint">{t("cmap.pickHint")}</span>
                    ) : (
                      <>
                        <span className="cm-detail-char" style={fontFamily ? { fontFamily } : undefined}>
                          {String.fromCodePoint(picked)}
                        </span>
                        <div className="cm-detail-meta">
                          <span className="cm-detail-code tabular">{fmt(picked)}</span>
                          {pickedName && <span className="cm-detail-name">{pickedName}</span>}
                          {pickedBlock && <span className="cm-detail-block">{pickedBlock.label}</span>}
                        </div>
                        <div className="cm-detail-actions">
                          <button className="exp-btn" onClick={() => copy(String.fromCodePoint(picked))}>
                            <Copy size={12} strokeWidth={1.7} /> {t("cmap.copyChar")}
                          </button>
                          <button className="exp-btn" onClick={() => copy(fmt(picked))}>
                            {t("cmap.copyCode")}
                          </button>
                        </div>
                      </>
                    )}
                  </div>
                </div>
              </div>
            )}
          </motion.div>
        </motion.div>
      )}
    </AnimatePresence>
  );
}
