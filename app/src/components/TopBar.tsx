import { AnimatePresence, motion } from "motion/react";
import {
  ArrowUpDown,
  Check,
  ChevronDown,
  Columns2,
  Filter,
  LayoutGrid,
  List,
  RefreshCw,
  Search,
  Type,
  X,
} from "lucide-react";
import { useState, type CSSProperties } from "react";
import { springSnappy, springSoft } from "../design/springs";
import { SIZES, useFontStore, type SortMode } from "../state/fontStore";
import type { Classification, FontFormat } from "../lib/ipc";
import { useT, type TKey } from "../lib/i18n";
import { TAG_GROUPS } from "../lib/tagTaxonomy";
import { SAMPLE_PRESETS } from "../lib/samplePresets";
import { BOOK_STYLES } from "../lib/bookStyle";

const SORT_MODES = ["name", "recent", "added", "rating", "styles", "size", "glyphs"] as const satisfies readonly SortMode[];

const SORT_KEYS: Record<SortMode, TKey> = {
  name: "sort.name",
  recent: "sort.recent",
  added: "sort.added",
  rating: "sort.rating",
  styles: "sort.styles",
  size: "sort.size",
  glyphs: "sort.glyphs",
};

const FORMAT_FILTERS = ["otf", "ttf", "woff", "woff2"] as const;

const FEATURE_FILTERS = ["swsh", "salt", "dlig", "smcp", "onum", "frac"] as const;
const INSTALL_FILTERS = ["all", "installed", "notInstalled"] as const;
const RATING_FILTERS = ["all", "top", "none"] as const;

function SortMenu() {
  const t = useT();
  const sort = useFontStore((s) => s.sort);
  const setSort = useFontStore((s) => s.setSort);
  const [open, setOpen] = useState(false);

  return (
    <div className="sort-wrap">
      <motion.button
        className={`sort-btn ${open ? "sort-open" : ""}`}
        onClick={() => setOpen((v) => !v)}
        aria-haspopup="listbox"
        aria-expanded={open}
        whileTap={{ scale: 0.96 }}
      >
        <ArrowUpDown size={13} strokeWidth={1.5} />
        <span>{t(SORT_KEYS[sort])}</span>
      </motion.button>
      <AnimatePresence>
        {open && (
          <>
            <div className="menu-backdrop" onClick={() => setOpen(false)} />
            <motion.ul
              className="sort-menu glass-e3"
              role="listbox"
              initial={{ opacity: 0, y: -6, scale: 0.97 }}
              animate={{ opacity: 1, y: 0, scale: 1 }}
              exit={{ opacity: 0, y: -4, scale: 0.98, transition: { duration: 0.13 } }}
              transition={springSnappy}
            >
              {SORT_MODES.map((mode) => (
                <li key={mode}>
                  <button
                    role="option"
                    aria-selected={sort === mode}
                    className={sort === mode ? "sort-item sort-active" : "sort-item"}
                    onClick={() => {
                      setSort(mode);
                      setOpen(false);
                    }}
                  >
                    <span>{t(SORT_KEYS[mode])}</span>
                    {sort === mode && <Check size={13} strokeWidth={2} />}
                  </button>
                </li>
              ))}
            </motion.ul>
          </>
        )}
      </AnimatePresence>
    </div>
  );
}

const CLASS_FILTERS = [
  "serif",
  "sans",
  "mono",
  "display",
  "script",
] as const satisfies readonly Classification[];

const SCRIPT_FILTERS = [
  "latin",
  "cyrillic",
  "greek",
  "arabic",
  "hebrew",
  "devanagari",
  "thai",
  "cjk",
  "korean",
  "vietnamese",
] as const;

function FilterMenu() {
  const t = useT();
  const classFilter = useFontStore((s) => s.classFilter);
  const toggleClassFilter = useFontStore((s) => s.toggleClassFilter);
  const scriptFilter = useFontStore((s) => s.scriptFilter);
  const toggleScriptFilter = useFontStore((s) => s.toggleScriptFilter);
  const bookFilter = useFontStore((s) => s.bookFilter);
  const toggleBookFilter = useFontStore((s) => s.toggleBookFilter);
  const clearBookFilter = useFontStore((s) => s.clearBookFilter);
  const missingOnly = useFontStore((s) => s.missingOnly);
  const setMissingOnly = useFontStore((s) => s.setMissingOnly);
  const glyphLoading = useFontStore((s) => s.glyphLoading);
  const variableOnly = useFontStore((s) => s.variableOnly);
  const setVariableOnly = useFontStore((s) => s.setVariableOnly);
  const toggleableOnly = useFontStore((s) => s.toggleableOnly);
  const setToggleableOnly = useFontStore((s) => s.setToggleableOnly);
  const formatFilter = useFontStore((s) => s.formatFilter);
  const toggleFormat = useFontStore((s) => s.toggleFormat);
  const featureFilter = useFontStore((s) => s.featureFilter);
  const toggleFeature = useFontStore((s) => s.toggleFeature);
  const installFilter = useFontStore((s) => s.installFilter);
  const setInstallFilter = useFontStore((s) => s.setInstallFilter);
  const charFilter = useFontStore((s) => s.charFilter);
  const charSearching = useFontStore((s) => s.charSearching);
  const setCharFilter = useFontStore((s) => s.setCharFilter);
  const ratingFilter = useFontStore((s) => s.ratingFilter);
  const setRatingFilter = useFontStore((s) => s.setRatingFilter);
  const tagFilter = useFontStore((s) => s.tagFilter);
  const toggleTagFilter = useFontStore((s) => s.toggleTagFilter);
  const clearTagFilter = useFontStore((s) => s.clearTagFilter);
  const untaggedOnly = useFontStore((s) => s.untaggedOnly);
  const setUntaggedOnly = useFontStore((s) => s.setUntaggedOnly);
  const [open, setOpen] = useState(false);

  const activeCount =
    classFilter.length +
    scriptFilter.length +
    bookFilter.length +
    tagFilter.length +
    (untaggedOnly ? 1 : 0) +
    (missingOnly ? 1 : 0) +
    formatFilter.length +
    featureFilter.length +
    (installFilter !== "all" ? 1 : 0) +
    (ratingFilter !== "all" ? 1 : 0) +
    (charFilter ? 1 : 0) +
    (variableOnly ? 1 : 0) +
    (toggleableOnly ? 1 : 0);

  return (
    <div className="sort-wrap">
      <motion.button
        className={`sort-btn ${open ? "sort-open" : ""} ${activeCount > 0 ? "filter-live" : ""}`}
        onClick={() => setOpen((v) => !v)}
        aria-haspopup="listbox"
        aria-expanded={open}
        aria-label={t("filter.aria")}
        whileTap={{ scale: 0.96 }}
      >
        <Filter size={13} strokeWidth={1.5} />
        {activeCount > 0 && <span className="filter-count tabular">{activeCount}</span>}
      </motion.button>
      <AnimatePresence>
        {open && (
          <>
            <div className="menu-backdrop" onClick={() => setOpen(false)} />
            <motion.ul
              className="sort-menu filter-menu glass-e3"
              role="listbox"
              aria-multiselectable
              initial={{ opacity: 0, y: -6, scale: 0.97 }}
              animate={{ opacity: 1, y: 0, scale: 1 }}
              exit={{ opacity: 0, y: -4, scale: 0.98, transition: { duration: 0.13 } }}
              transition={springSnappy}
            >
              <li className="filter-heading" role="none">
                {t("filter.installState")}
              </li>
              {INSTALL_FILTERS.map((value) => (
                <li key={value}>
                  <button
                    role="option"
                    aria-selected={installFilter === value}
                    className={installFilter === value ? "sort-item sort-active" : "sort-item"}
                    onClick={() => setInstallFilter(value)}
                  >
                    <span>{t(`install.${value}`)}</span>
                    {installFilter === value && <Check size={13} strokeWidth={2} />}
                  </button>
                </li>
              ))}
              <li className="sort-sep" role="none" />
              <li className="filter-heading" role="none">
                {t("filter.rating")}
              </li>
              {RATING_FILTERS.map((value) => (
                <li key={value}>
                  <button
                    role="option"
                    aria-selected={ratingFilter === value}
                    className={ratingFilter === value ? "sort-item sort-active" : "sort-item"}
                    onClick={() => setRatingFilter(value)}
                  >
                    <span>{t(`rating.${value}`)}</span>
                    {ratingFilter === value && <Check size={13} strokeWidth={2} />}
                  </button>
                </li>
              ))}
              <li className="sort-sep" role="none" />
              <li className="filter-heading" role="none">
                {t("filter.classification")}
              </li>
              {CLASS_FILTERS.map((c) => (
                <li key={c}>
                  <button
                    role="option"
                    aria-selected={classFilter.includes(c)}
                    className={classFilter.includes(c) ? "sort-item sort-active" : "sort-item"}
                    onClick={() => toggleClassFilter(c)}
                  >
                    <span>{t(`class.${c}`)}</span>
                    {classFilter.includes(c) && <Check size={13} strokeWidth={2} />}
                  </button>
                </li>
              ))}
              <li className="sort-sep" role="none" />
              <li className="filter-heading" role="none">
                {t("filter.bookStyle")}
              </li>
              <li className="filter-taggroup" role="none">
                <div className="filter-tagchips">
                  {BOOK_STYLES.map((style) => {
                    const on = bookFilter.includes(style);
                    return (
                      <button
                        key={style}
                        role="option"
                        aria-selected={on}
                        className={`filter-tagchip ${on ? "filter-tagchip-on" : ""}`}
                        onClick={() => toggleBookFilter(style)}
                      >
                        {t(`book.${style}` as TKey)}
                      </button>
                    );
                  })}
                </div>
              </li>
              {bookFilter.length > 0 && (
                <li>
                  <button
                    role="option"
                    aria-selected={false}
                    className="sort-item filter-clear"
                    onClick={clearBookFilter}
                  >
                    <span>{t("filter.bookClear")}</span>
                    <X size={13} strokeWidth={2} />
                  </button>
                </li>
              )}
              <li className="sort-sep" role="none" />
              <li className="filter-heading" role="none">
                {t("filter.tags")}
              </li>
              {TAG_GROUPS.map((group) => (
                <li className="filter-taggroup" role="none" key={group.id}>
                  <div className="filter-taggroup-label">{t(group.labelKey)}</div>
                  <div className="filter-tagchips">
                    {group.tags.map((entry) => {
                      const on = tagFilter.includes(entry.value);
                      return (
                        <button
                          key={entry.value}
                          role="option"
                          aria-selected={on}
                          className={`filter-tagchip ${on ? "filter-tagchip-on" : ""}`}
                          onClick={() => toggleTagFilter(entry.value)}
                        >
                          {t(entry.labelKey)}
                        </button>
                      );
                    })}
                  </div>
                </li>
              ))}
              <li>
                <button
                  role="option"
                  aria-selected={untaggedOnly}
                  className={untaggedOnly ? "sort-item sort-active" : "sort-item"}
                  onClick={() => setUntaggedOnly(!untaggedOnly)}
                >
                  <span>{t("filter.untaggedOnly")}</span>
                  {untaggedOnly && <Check size={13} strokeWidth={2} />}
                </button>
              </li>
              {(tagFilter.length > 0 || untaggedOnly) && (
                <li>
                  <button
                    role="option"
                    aria-selected={false}
                    className="sort-item filter-clear"
                    onClick={clearTagFilter}
                  >
                    <span>{t("filter.clearTags")}</span>
                    <X size={13} strokeWidth={2} />
                  </button>
                </li>
              )}
              <li className="sort-sep" role="none" />
              <li className="filter-heading" role="none">
                {t("filter.languageSupport")}
              </li>
              {SCRIPT_FILTERS.map((sc) => (
                <li key={sc}>
                  <button
                    role="option"
                    aria-selected={scriptFilter.includes(sc)}
                    className={scriptFilter.includes(sc) ? "sort-item sort-active" : "sort-item"}
                    onClick={() => toggleScriptFilter(sc)}
                  >
                    <span>{t(`script.${sc}`)}</span>
                    {scriptFilter.includes(sc) && <Check size={13} strokeWidth={2} />}
                  </button>
                </li>
              ))}
              <li className="sort-sep" role="none" />
              <li className="filter-heading" role="none">
                {t("filter.fileFormat")}
              </li>
              {FORMAT_FILTERS.map((fmt) => (
                <li key={fmt}>
                  <button
                    role="option"
                    aria-selected={formatFilter.includes(fmt)}
                    className={formatFilter.includes(fmt) ? "sort-item sort-active" : "sort-item"}
                    onClick={() => toggleFormat(fmt as FontFormat)}
                  >
                    <span>{t(`format.${fmt}`)}</span>
                    {formatFilter.includes(fmt) && <Check size={13} strokeWidth={2} />}
                  </button>
                </li>
              ))}
              <li className="sort-sep" role="none" />
              <li className="filter-heading" role="none">
                {t("filter.typography")}
              </li>
              {FEATURE_FILTERS.map((tag) => (
                <li key={tag}>
                  <button
                    role="option"
                    aria-selected={featureFilter.includes(tag)}
                    className={featureFilter.includes(tag) ? "sort-item sort-active" : "sort-item"}
                    onClick={() => toggleFeature(tag)}
                  >
                    <span>{t(`feat.${tag}`)}</span>
                    {featureFilter.includes(tag) && <Check size={13} strokeWidth={2} />}
                  </button>
                </li>
              ))}
              <li className="sort-sep" role="none" />
              <li className="filter-heading" role="none">
                {t("filter.containsChar")}
              </li>
              <li className="filter-charrow" role="none">
                <input
                  className="filter-charinput"
                  value={charFilter}
                  maxLength={2}
                  spellCheck={false}
                  placeholder={t("filter.containsCharPlaceholder")}
                  aria-label={t("filter.containsChar")}
                  onChange={(e) => setCharFilter(e.target.value)}
                />
                <span className="filter-charnote">
                  {charSearching
                    ? t("filter.containsCharSearching")
                    : charFilter
                      ? `U+${charFilter.codePointAt(0)!.toString(16).toUpperCase().padStart(4, "0")}`
                      : t("filter.containsCharHint")}
                </span>
                {charFilter && (
                  <button
                    className="search-clear"
                    onClick={() => setCharFilter("")}
                    aria-label={t("filter.containsCharClear")}
                  >
                    <X size={13} strokeWidth={2} />
                  </button>
                )}
              </li>
              <li className="sort-sep" role="none" />
              <li>
                <button
                  role="option"
                  aria-selected={variableOnly}
                  className={variableOnly ? "sort-item sort-active" : "sort-item"}
                  onClick={() => setVariableOnly(!variableOnly)}
                >
                  <span>{t("filter.variableOnly")}</span>
                  {variableOnly && <Check size={13} strokeWidth={2} />}
                </button>
              </li>
              <li>
                <button
                  role="option"
                  aria-selected={toggleableOnly}
                  className={toggleableOnly ? "sort-item sort-active" : "sort-item"}
                  onClick={() => setToggleableOnly(!toggleableOnly)}
                >
                  <span>{t("filter.toggleableOnly")}</span>
                  {toggleableOnly && <Check size={13} strokeWidth={2} />}
                </button>
              </li>
              <li>
                <button
                  role="option"
                  aria-selected={missingOnly}
                  className={missingOnly ? "sort-item sort-active" : "sort-item"}
                  onClick={() => setMissingOnly(!missingOnly)}
                >
                  <span>{t("filter.missingOnly")}</span>
                  {missingOnly && <Check size={13} strokeWidth={2} />}
                </button>
              </li>
              {missingOnly && glyphLoading && (
                <li className="filter-hint" role="none">
                  {t("filter.missingSearching")}
                </li>
              )}
            </motion.ul>
          </>
        )}
      </AnimatePresence>
    </div>
  );
}

function RescanButton() {
  const t = useT();
  const rescan = useFontStore((s) => s.rescan);
  const scanning = useFontStore((s) => s.phase === "scanning");
  return (
    <motion.button
      className="rescan-btn"
      aria-label={t("top.rescan")}
      disabled={scanning}
      onClick={() => void rescan()}
      whileTap={{ scale: 0.94 }}
    >
      <motion.span
        className="rescan-icon"
        animate={scanning ? { rotate: 360 } : { rotate: 0 }}
        transition={
          scanning
            ? { duration: 1.2, repeat: Infinity, ease: "linear" }
            : { duration: 0.2 }
        }
      >
        <RefreshCw size={14} strokeWidth={1.5} />
      </motion.span>
    </motion.button>
  );
}

function CompareButton() {
  const t = useT();
  const selection = useFontStore((s) => s.selection);
  const picking = useFontStore((s) => s.comparePicking);
  const openCompare = useFontStore((s) => s.openCompare);
  const setComparePicking = useFontStore((s) => s.setComparePicking);
  const armed = selection.length >= 2;
  return (
    <motion.button
      className={`compare-btn ${armed ? "compare-armed" : picking ? "compare-picking" : ""}`}
      aria-label={
        armed
          ? t("compare.ariaArmed", { count: Math.min(selection.length, 4) })
          : picking
            ? t("compare.ariaCancelPicking")
            : t("compare.ariaIdle")
      }
      onClick={() => {
        if (armed) openCompare(selection);
        else setComparePicking(!picking);
      }}
      whileHover={{ y: -1 }}
      whileTap={{ scale: 0.96 }}
    >
      <Columns2 size={14} strokeWidth={1.5} />
      <span>
        {armed
          ? t("compare.buttonCount", { count: Math.min(selection.length, 4) })
          : picking
            ? t("compare.picking", { count: selection.length })
            : t("compare.button")}
      </span>
    </motion.button>
  );
}

function PresetMenu() {
  const t = useT();
  const sampleText = useFontStore((s) => s.sampleText);
  const samplePreset = useFontStore((s) => s.samplePreset);
  const sampleHistory = useFontStore((s) => s.sampleHistory);
  const applySamplePreset = useFontStore((s) => s.applySamplePreset);
  const setSampleText = useFontStore((s) => s.setSampleText);
  const clearSampleHistory = useFontStore((s) => s.clearSampleHistory);
  const [open, setOpen] = useState(false);

  return (
    <div className="sort-wrap preset-wrap">
      <motion.button
        className={`preset-btn ${open ? "sort-open" : ""} ${samplePreset ? "preset-on" : ""}`}
        onClick={() => setOpen((v) => !v)}
        aria-haspopup="menu"
        aria-expanded={open}
        aria-label={t("top.presetsAria")}
        title={t("top.presets")}
        whileTap={{ scale: 0.94 }}
      >
        <Type size={14} strokeWidth={1.5} />
        <ChevronDown size={11} strokeWidth={2} />
      </motion.button>
      <AnimatePresence>
        {open && (
          <>
            <div className="menu-backdrop" onClick={() => setOpen(false)} />
            <motion.ul
              className="sort-menu preset-menu glass-e3"
              role="menu"
              initial={{ opacity: 0, y: -6, scale: 0.97 }}
              animate={{ opacity: 1, y: 0, scale: 1 }}
              exit={{ opacity: 0, y: -4, scale: 0.98, transition: { duration: 0.13 } }}
              transition={springSnappy}
            >
              <li className="filter-heading" role="none">
                {t("top.presets")}
              </li>
              {SAMPLE_PRESETS.map((p) => {
                const on = samplePreset === p.id;
                return (
                  <li key={p.id} role="none">
                    <button
                      role="menuitem"
                      className={on ? "sort-item sort-active" : "sort-item"}
                      onClick={() => {
                        applySamplePreset(p.id, p.text);
                        setOpen(false);
                      }}
                    >
                      <span>{t(p.labelKey)}</span>
                      {on && <Check size={13} strokeWidth={2} />}
                    </button>
                    <div className="preset-echo">{p.text}</div>
                  </li>
                );
              })}
              {sampleHistory.length > 0 && (
                <>
                  <li className="sort-sep" role="none" />
                  <li className="filter-heading" role="none">
                    {t("top.history")}
                  </li>
                  {sampleHistory.slice(0, 6).map((h) => (
                    <li key={h} role="none">
                      <button
                        role="menuitem"
                        className={`sort-item preset-history ${
                          sampleText === h ? "sort-active" : ""
                        }`}
                        onClick={() => {
                          setSampleText(h);
                          setOpen(false);
                        }}
                      >
                        <span className="preset-history-text">{h}</span>
                        {sampleText === h && <Check size={13} strokeWidth={2} />}
                      </button>
                    </li>
                  ))}
                  <li role="none">
                    <button
                      role="menuitem"
                      className="sort-item filter-clear"
                      onClick={() => clearSampleHistory()}
                    >
                      <span>{t("top.clearHistory")}</span>
                      <X size={13} strokeWidth={2} />
                    </button>
                  </li>
                </>
              )}
            </motion.ul>
          </>
        )}
      </AnimatePresence>
    </div>
  );
}

function TypeOptions() {
  const t = useT();
  const lineHeight = useFontStore((s) => s.lineHeight);
  const setLineHeight = useFontStore((s) => s.setLineHeight);
  const vertical = useFontStore((s) => s.vertical);
  const setVertical = useFontStore((s) => s.setVertical);
  const [open, setOpen] = useState(false);

  return (
    <div className="sort-wrap">
      <motion.button
        className={`type-more ${open ? "sort-open" : ""}`}
        onClick={() => setOpen((v) => !v)}
        aria-haspopup="menu"
        aria-expanded={open}
        aria-label={t("top.typeOptions")}
        title={t("top.typeOptions")}
        whileTap={{ scale: 0.94 }}
      >
        <ChevronDown size={12} strokeWidth={2} />
      </motion.button>
      <AnimatePresence>
        {open && (
          <>
            <div className="menu-backdrop" onClick={() => setOpen(false)} />
            <motion.div
              className="sort-menu type-menu glass-e3"
              role="group"
              initial={{ opacity: 0, y: -6, scale: 0.97 }}
              animate={{ opacity: 1, y: 0, scale: 1 }}
              exit={{ opacity: 0, y: -4, scale: 0.98, transition: { duration: 0.13 } }}
              transition={springSnappy}
            >
              <div className="type-row">
                <span className="type-label">{t("top.lineHeight")}</span>
                <input
                  type="range"
                  min={100}
                  max={240}
                  step={5}
                  value={Math.round(lineHeight * 100)}
                  onChange={(e) => setLineHeight(Number(e.target.value) / 100)}
                  aria-label={t("top.lineHeight")}
                />
                <span className="type-value tabular">{lineHeight.toFixed(2)}</span>
              </div>
              <div className="type-row">
                <span className="type-label">{t("top.layout")}</span>
                <div className="type-seg" role="group" aria-label={t("top.layout")}>
                  <button
                    className={vertical ? "" : "on"}
                    aria-pressed={!vertical}
                    onClick={() => setVertical(false)}
                  >
                    {t("top.horizontal")}
                  </button>
                  <button
                    className={vertical ? "on" : ""}
                    aria-pressed={vertical}
                    onClick={() => setVertical(true)}
                  >
                    {t("top.vertical")}
                  </button>
                </div>
              </div>
            </motion.div>
          </>
        )}
      </AnimatePresence>
    </div>
  );
}

export function TopBar() {
  const t = useT();
  const sampleText = useFontStore((s) => s.sampleText);
  const setSampleText = useFontStore((s) => s.setSampleText);
  const sizeIndex = useFontStore((s) => s.sizeIndex);
  const setSizeIndex = useFontStore((s) => s.setSizeIndex);
  const viewMode = useFontStore((s) => s.viewMode);
  const setViewMode = useFontStore((s) => s.setViewMode);
  const search = useFontStore((s) => s.search);
  const setSearch = useFontStore((s) => s.setSearch);

  const fill = (sizeIndex / (SIZES.length - 1)) * 100;

  return (
    <motion.header
      className="topbar"
      initial={{ y: -32, opacity: 0 }}
      animate={{ y: 0, opacity: 1 }}
      transition={{ ...springSoft, delay: 0.06 }}
    >
      <div className="topbar-field topbar-search">
        <Search size={14} strokeWidth={1.5} className="field-icon" />
        <input
          id="global-search"
          value={search}
          onChange={(e) => setSearch(e.target.value)}
          placeholder={t("top.searchPlaceholder")}
          aria-label={t("top.searchAria")}
          spellCheck={false}
        />
        {search.length > 0 && (
          <button
            className="search-clear"
            onClick={() => setSearch("")}
            aria-label={t("top.searchClear")}
          >
            <X size={13} strokeWidth={2} />
          </button>
        )}
      </div>

      <div className="topbar-field topbar-sample">
        <PresetMenu />
        <input
          value={sampleText}
          onChange={(e) => setSampleText(e.target.value)}
          placeholder={t("top.samplePlaceholder")}
          aria-label={t("top.sampleAria")}
          spellCheck={false}
        />
      </div>

      <div className="topbar-size" style={{ "--fill": `${fill}%` } as CSSProperties}>
        <input
          type="range"
          min={0}
          max={SIZES.length - 1}
          step={1}
          value={sizeIndex}
          onChange={(e) => setSizeIndex(Number(e.target.value))}
          aria-label={t("top.sizeAria")}
        />
        <span className="size-value tabular">{SIZES[sizeIndex]}</span>
        <TypeOptions />
      </div>

      <FilterMenu />

      <SortMenu />

      <CompareButton />

      <RescanButton />

      <div className="view-toggle" role="tablist" aria-label={t("top.viewMode")}>
        {(["grid", "list"] as const).map((mode) => (
          <motion.button
            key={mode}
            role="tab"
            aria-selected={viewMode === mode}
            className={`view-btn ${viewMode === mode ? "view-btn-active" : ""}`}
            onClick={() => setViewMode(mode)}
            whileTap={{ scale: 0.94 }}
          >
            {mode === "grid" ? (
              <LayoutGrid size={15} strokeWidth={1.5} />
            ) : (
              <List size={15} strokeWidth={1.5} />
            )}
          </motion.button>
        ))}
      </div>
    </motion.header>
  );
}
