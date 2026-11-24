import { motion } from "motion/react";
import { CheckCircle2, ChevronDown, CircleDashed, Clock, FolderCog, FolderOpen, Keyboard, Library, MoreHorizontal, Plus, Sparkles, Star, Tag, Trash2 } from "lucide-react";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { readTextFile } from "@tauri-apps/plugin-fs";
import { toast } from "../design/primitives/Toast";
import { useEffect, useMemo, useRef, useState } from "react";
import { openContextMenu } from "../design/primitives/ContextMenu";
import { spring, springSoft, staggerDelay } from "../design/springs";
import { allTags, familiesFor, useFontStore, type Family, type Nav } from "../state/fontStore";
import { BOOK_STYLES, bookStyleOf, isChinese, type BookStyle } from "../lib/bookStyle";
import { exportFontList } from "../lib/menus";
import { t as translate, useT } from "../lib/i18n";
import { TAG_GROUPS, groupIdOfTag } from "../lib/tagTaxonomy";

function navKey(n: Nav): string {
  if (n.kind === "tag") return `tag:${n.tag}`;
  if (n.kind === "collection") return `col:${n.name}`;
  if (n.kind === "favorites") return "favorites";
  return n.kind;
}

function NavRow({
  nav,
  icon,
  label,
  count,
  index,
  onContextMenu,
}: {
  nav: Nav;
  icon: React.ReactNode;
  label: string;
  count?: number;
  index: number;
  onContextMenu?: (e: React.MouseEvent) => void;
}) {
  const current = useFontStore((s) => s.nav);
  const setNav = useFontStore((s) => s.setNav);
  const active = navKey(current) === navKey(nav);
  return (
    <motion.button
      className={`nav-row ${active ? "nav-active" : ""}`}
      onClick={() => setNav(nav)}
      onContextMenu={onContextMenu}
      initial={{ opacity: 0, x: -12 }}
      animate={{ opacity: 1, x: 0 }}
      transition={{ ...springSoft, delay: staggerDelay(index) }}
      whileTap={{ scale: 0.97 }}
    >
      {active && (
        <motion.span className="nav-pill" layoutId="nav-pill" transition={spring} />
      )}
      <span className="nav-icon">{icon}</span>
      <span className="nav-label">{label}</span>
      {count !== undefined && <span className="nav-count tabular">{count}</span>}
    </motion.button>
  );
}

/**
 * 筛选行：没有图标，比导航行矮一档 —— 视觉上自动退到第二层，
 * 一眼能分清「去哪看」和「筛什么」。
 */
function FilterRow({
  label,
  count,
  on,
  onClick,
  disabled,
}: {
  label: string;
  count: number;
  on: boolean;
  onClick: () => void;
  disabled?: boolean;
}) {
  return (
    <button
      className={`side-filter ${on ? "side-filter-on" : ""}`}
      onClick={onClick}
      disabled={disabled}
      aria-pressed={on}
    >
      <span className="side-filter-label">{label}</span>
      <span className="side-filter-count tabular">{count}</span>
    </button>
  );
}

/** 书体 / 特征用两列方块，比一行一条省一半高度。 */
function FilterChip({
  label,
  count,
  on,
  onClick,
  disabled,
}: {
  label: string;
  count: number;
  on: boolean;
  onClick: () => void;
  disabled?: boolean;
}) {
  return (
    <button
      className={`side-chip ${on ? "side-chip-on" : ""}`}
      onClick={onClick}
      disabled={disabled}
      aria-pressed={on}
    >
      <span className="side-chip-label">{label}</span>
      <span className="side-chip-count tabular">{count}</span>
    </button>
  );
}

function NewCollectionInput({ onDone }: { onDone: () => void }) {
  const t = useT();
  const createCollection = useFontStore((s) => s.createCollection);
  const [name, setName] = useState("");
  const ref = useRef<HTMLInputElement>(null);

  useEffect(() => ref.current?.focus(), []);

  const commit = () => {
    if (name.trim()) void createCollection(name);
    onDone();
  };

  return (
    <motion.div
      className="collection-input"
      initial={{ opacity: 0, y: -6 }}
      animate={{ opacity: 1, y: 0 }}
      transition={spring}
    >
      <FolderOpen size={15} strokeWidth={1.5} />
      <input
        ref={ref}
        value={name}
        onChange={(e) => setName(e.target.value)}
        onKeyDown={(e) => {
          if (e.key === "Enter") commit();
          if (e.key === "Escape") onDone();
        }}
        onBlur={commit}
        placeholder={t("side.collectionPlaceholder")}
        aria-label={t("side.newCollectionAria")}
        spellCheck={false}
      />
    </motion.div>
  );
}

function RenameInput({
  initial,
  onCommit,
  onCancel,
}: {
  initial: string;
  onCommit: (name: string) => void;
  onCancel: () => void;
}) {
  const t = useT();
  const [name, setName] = useState(initial);
  const ref = useRef<HTMLInputElement>(null);
  useEffect(() => {
    ref.current?.focus();
    ref.current?.select();
  }, []);
  return (
    <div className="collection-input">
      <FolderOpen size={15} strokeWidth={1.5} />
      <input
        ref={ref}
        value={name}
        onChange={(e) => setName(e.target.value)}
        onKeyDown={(e) => {
          if (e.key === "Enter") onCommit(name);
          if (e.key === "Escape") onCancel();
        }}
        onBlur={() => onCommit(name)}
        aria-label={t("side.renameAria")}
        spellCheck={false}
      />
    </div>
  );
}

export function Sidebar() {
  const t = useT();
  const fonts = useFontStore((s) => s.fonts);
  const tags = useFontStore((s) => s.tags);
  const collections = useFontStore((s) => s.collections);
  const favorites = useFontStore((s) => s.favorites);
  const trash = useFontStore((s) => s.trash);
  const deleteCollection = useFontStore((s) => s.deleteCollection);
  const renameCollection = useFontStore((s) => s.renameCollection);
  const setFamilyTags = useFontStore((s) => s.setFamilyTags);
  const pendingCollectionFor = useFontStore((s) => s.pendingCollectionFor);
  const sessionActivated = useFontStore((s) => s.sessionActivated);
  const bookFilter = useFontStore((s) => s.bookFilter);
  const toggleBookFilter = useFontStore((s) => s.toggleBookFilter);
  const rangeFilter = useFontStore((s) => s.rangeFilter);
  const setRangeFilter = useFontStore((s) => s.setRangeFilter);
  const variableOnly = useFontStore((s) => s.variableOnly);
  const multiStyleOnly = useFontStore((s) => s.multiStyleOnly);
  const bigCharsetOnly = useFontStore((s) => s.bigCharsetOnly);
  const classFilter = useFontStore((s) => s.classFilter);
  const toggleSidebarAttribute = useFontStore((s) => s.toggleSidebarAttribute);
  const setNav = useFontStore((s) => s.setNav);

  const [creating, setCreating] = useState(false);
  const [renaming, setRenaming] = useState<string | null>(null);

  useEffect(() => {
    if (pendingCollectionFor) setCreating(true);
  }, [pendingCollectionFor]);

  // 字体库上万条时，每次渲染重建家族表会明显掉帧 —— 算一次，只跟着 fonts/tags 变。
  const familyList = useMemo(() => [...familiesFor(fonts, tags).values()], [fonts, tags]);
  const sessionSet = useMemo(() => new Set(sessionActivated), [sessionActivated]);

  const stats = useMemo(() => {
    const book: Partial<Record<BookStyle, number>> = {};
    let installed = 0;
    let notInstalled = 0;
    let inSession = 0;
    let cjk = 0;
    let variable = 0;
    let multiStyle = 0;
    let mono = 0;
    let big = 0;
    for (const f of familyList) {
      const session = sessionSet.has(f.name);
      if (session) inSession++;
      if (f.active && !session) installed++;
      if (!f.active) notInstalled++;
      if (isChinese(f)) cjk++;
      const style = bookStyleOf(f);
      if (style) book[style] = (book[style] ?? 0) + 1;
      if (f.isVariable) variable++;
      if (f.faces.length > 1) multiStyle++;
      if (f.classification === "mono") mono++;
      if (f.faces.some((x) => x.glyphCount >= 20000)) big++;
    }
    return {
      installed,
      notInstalled,
      inSession,
      cjk,
      latin: familyList.length - cjk,
      book,
      variable,
      multiStyle,
      mono,
      big,
    };
  }, [familyList, sessionSet]);

  const monospaceOn = classFilter.includes("mono");

  const activeFilters =
    (rangeFilter !== "all" ? 1 : 0) +
    bookFilter.length +
    (variableOnly ? 1 : 0) +
    (multiStyleOnly ? 1 : 0) +
    (monospaceOn ? 1 : 0) +
    (bigCharsetOnly ? 1 : 0);

  const clearFilters = () => {
    useFontStore.setState({
      rangeFilter: "all",
      bookFilter: [],
      variableOnly: false,
      multiStyleOnly: false,
      bigCharsetOnly: false,
      classFilter: classFilter.filter((c) => c !== "mono"),
    });
  };

  const tagCounts = allTags(tags);
  const collapsed = useFontStore((s) => s.collapsedSections);
  const toggleSection = useFontStore((s) => s.toggleSection);
  const collectionNames = Object.keys(collections).sort((a, b) => a.localeCompare(b));

  const removeTagEverywhere = (tag: string) => {
    for (const [family, list] of Object.entries(tags)) {
      if (list.includes(tag)) {
        void setFamilyTags(family, list.filter((t) => t !== tag));
      }
    }
  };

  // 回读 AI 名录：支持 {family: tags[]} 直映射，或 [{family, tags}] 行数组；
  // 若读的是导出中心生成的清单 JSON（含 ExportRow 数组），按 family 聚合。
  const importAiCatalog = async () => {
    const path = await openDialog({
      title: t("side.importAiCatalog"),
      filters: [{ name: "JSON", extensions: ["json"] }],
    });
    if (!path) return;
    try {
      const text = await readTextFile(path);
      const data: unknown = JSON.parse(text);
      let entries: Record<string, string[]> = {};
      if (Array.isArray(data)) {
        for (const row of data as { family?: unknown; tags?: unknown }[]) {
          if (typeof row?.family !== "string") continue;
          const cur = entries[row.family] ?? [];
          const add = Array.isArray(row.tags)
            ? row.tags.filter((x): x is string => typeof x === "string")
            : [];
          entries[row.family] = [...new Set([...cur, ...add])];
        }
      } else if (data && typeof data === "object") {
        for (const [k, v] of Object.entries(data as Record<string, unknown>)) {
          if (Array.isArray(v)) {
            entries[k] = v.filter((x): x is string => typeof x === "string");
          }
        }
      }
      const count = Object.keys(entries).length;
      if (count === 0) {
        toast.error(t("toast.importTagsFailed"), t("toast.importTagsEmpty"));
        return;
      }
      await useFontStore.getState().importTags(entries);
    } catch (e) {
      toast.error(t("toast.importTagsFailed"), String(e));
    }
  };

  const openToolsMenu = (e: React.MouseEvent) => {
    openContextMenu(e, [
      { label: translate("side.usage"), action: () => setNav({ kind: "usage" }) },
      { label: translate("side.duplicates"), action: () => setNav({ kind: "duplicates" }) },
      { label: translate("side.references"), action: () => setNav({ kind: "references" }) },
      { label: translate("side.organize"), action: () => setNav({ kind: "organize" }) },
      { label: translate("side.backup"), action: () => setNav({ kind: "backup" }) },
      { kind: "separator" },
      { label: translate("side.about"), action: () => setNav({ kind: "about" }) },
      {
        label: translate("side.shortcuts"),
        action: () => useFontStore.getState().setHelpOpen(true),
      },
    ]);
  };

  let i = 0;
  return (
    <motion.aside
      className="sidebar"
      initial={{ x: -48, opacity: 0 }}
      animate={{ x: 0, opacity: 1 }}
      transition={springSoft}
    >
      <div className="sidebar-scroll">
        {activeFilters > 0 && (
          <div className="side-clearbar">
            <span className="side-clearbar-text">
              {t("side.filtersActive", { count: activeFilters })}
            </span>
            <button className="side-clearbar-btn" onClick={clearFilters}>
              {t("side.clearFilters")}
            </button>
          </div>
        )}

        <div className={`sidebar-section ${collapsed.includes("browse") ? "sidebar-collapsed" : ""}`}>
          <div className="sidebar-heading sidebar-heading-row">
            <button
              className="sidebar-toggle"
              onClick={() => toggleSection("browse")}
              aria-expanded={!collapsed.includes("browse")}
            >
              <span className="sidebar-chevron">
                <ChevronDown size={12} strokeWidth={2} />
              </span>
              <span>{t("side.browse")}</span>
            </button>
          </div>
          {!collapsed.includes("browse") && (
          <>
          <NavRow
            nav={{ kind: "library" }}
            icon={<Library size={15} strokeWidth={1.5} />}
            label={t("side.allFonts")}
            count={familyList.length}
            index={i++}
          />
          <NavRow
            nav={{ kind: "installed" }}
            icon={<CheckCircle2 size={15} strokeWidth={1.5} />}
            label={t("side.installed")}
            count={stats.installed}
            index={i++}
          />
          <NavRow
            nav={{ kind: "notInstalled" }}
            icon={<CircleDashed size={15} strokeWidth={1.5} />}
            label={t("side.notInstalled")}
            count={stats.notInstalled}
            index={i++}
          />
          <NavRow
            nav={{ kind: "favorites" }}
            icon={<Star size={15} strokeWidth={1.5} />}
            label={t("side.favorites")}
            count={favorites.length}
            index={i++}
          />
          {stats.inSession > 0 && (
            <NavRow
              nav={{ kind: "activatedSession" }}
              icon={<Clock size={15} strokeWidth={1.5} />}
              label={t("side.activatedUntilClose")}
              count={stats.inSession}
              index={i++}
            />
          )}
          </>
          )}
        </div>

        <div className={`sidebar-section ${collapsed.includes("range") ? "sidebar-collapsed" : ""}`}>
          <div className="sidebar-heading sidebar-heading-row">
            <button
              className="sidebar-toggle"
              onClick={() => toggleSection("range")}
              aria-expanded={!collapsed.includes("range")}
            >
              <span className="sidebar-chevron">
                <ChevronDown size={12} strokeWidth={2} />
              </span>
              <span>{t("side.range")}</span>
            </button>
          </div>
          {!collapsed.includes("range") && (
            <>
              <FilterRow
                label={t("side.rangeCjk")}
                count={stats.cjk}
                on={rangeFilter === "cjk"}
                onClick={() => setRangeFilter("cjk")}
              />
              <FilterRow
                label={t("side.rangeLatin")}
                count={stats.latin}
                on={rangeFilter === "latin"}
                onClick={() => setRangeFilter("latin")}
              />
            </>
          )}
        </div>

        <div className={`sidebar-section ${collapsed.includes("bookStyle") ? "sidebar-collapsed" : ""}`}>
          <div className="sidebar-heading sidebar-heading-row">
            <button
              className="sidebar-toggle"
              onClick={() => toggleSection("bookStyle")}
              aria-expanded={!collapsed.includes("bookStyle")}
            >
              <span className="sidebar-chevron">
                <ChevronDown size={12} strokeWidth={2} />
              </span>
              <span>{t("side.bookStyle")}</span>
            </button>
          </div>
          {!collapsed.includes("bookStyle") && (
            <div className="side-chip-grid">
              {BOOK_STYLES.map((style) => (
                <FilterChip
                  key={style}
                  label={t(`book.${style}`)}
                  count={stats.book[style] ?? 0}
                  on={bookFilter.includes(style)}
                  onClick={() => toggleBookFilter(style)}
                  disabled={!stats.book[style]}
                />
              ))}
            </div>
          )}
        </div>

        <div className={`sidebar-section ${collapsed.includes("attrs") ? "sidebar-collapsed" : ""}`}>
          <div className="sidebar-heading sidebar-heading-row">
            <button
              className="sidebar-toggle"
              onClick={() => toggleSection("attrs")}
              aria-expanded={!collapsed.includes("attrs")}
            >
              <span className="sidebar-chevron">
                <ChevronDown size={12} strokeWidth={2} />
              </span>
              <span>{t("side.attrs")}</span>
            </button>
          </div>
          {!collapsed.includes("attrs") && (
            <div className="side-chip-grid">
              <FilterChip
                label={t("filter.variableShort")}
                count={stats.variable}
                on={variableOnly}
                onClick={() => toggleSidebarAttribute("variable")}
              />
              <FilterChip
                label={t("filter.multiStyle")}
                count={stats.multiStyle}
                on={multiStyleOnly}
                onClick={() => toggleSidebarAttribute("multiStyle")}
              />
              <FilterChip
                label={t("filter.monospace")}
                count={stats.mono}
                on={monospaceOn}
                onClick={() => toggleSidebarAttribute("mono")}
              />
              <FilterChip
                label={t("filter.bigCharset")}
                count={stats.big}
                on={bigCharsetOnly}
                onClick={() => toggleSidebarAttribute("bigCharset")}
              />
            </div>
          )}
        </div>

        <div className={`sidebar-section ${collapsed.includes("collections") ? "sidebar-collapsed" : ""}`}>
          <div className="sidebar-heading sidebar-heading-row">
            <button
              className="sidebar-toggle"
              onClick={() => toggleSection("collections")}
              aria-expanded={!collapsed.includes("collections")}
            >
              <span className="sidebar-chevron">
                <ChevronDown size={12} strokeWidth={2} />
              </span>
              <span>{t("side.collections")}</span>
            </button>
            <motion.button
              className="sidebar-add"
              aria-label={t("side.newCollection")}
              onClick={() => setCreating(true)}
              whileTap={{ scale: 0.9 }}
            >
              <Plus size={13} strokeWidth={2} />
            </motion.button>
          </div>
          {creating && (
            <NewCollectionInput
              onDone={() => {
                setCreating(false);
                useFontStore.setState({ pendingCollectionFor: null });
              }}
            />
          )}
          {collectionNames.map((name) =>
            renaming === name ? (
              <RenameInput
                key={name}
                initial={name}
                onCommit={(next) => {
                  setRenaming(null);
                  void renameCollection(name, next);
                }}
                onCancel={() => setRenaming(null)}
              />
            ) : (
              <NavRow
                key={name}
                nav={{ kind: "collection", name }}
                icon={<FolderOpen size={15} strokeWidth={1.5} />}
                label={name}
                count={(collections[name] ?? []).length}
                index={i++}
                onContextMenu={(e) =>
                  openContextMenu(e, [
                    { label: translate("menu.rename"), action: () => setRenaming(name) },
                    {
                      label: translate("menu.exportFontList"),
                      action: () => {
                        const map = familiesFor(fonts, tags);
                        const fams = (collections[name] ?? [])
                          .map((n) => map.get(n))
                          .filter((f): f is Family => Boolean(f));
                        void exportFontList(fams, name);
                      },
                    },
                    {
                      label: translate("menu.exportData"),
                      action: () =>
                        useFontStore.getState().openExport(collections[name] ?? [], name),
                    },
                    { kind: "separator" },
                    {
                      label: translate("menu.deleteCollection"),
                      danger: true,
                      action: () => void deleteCollection(name),
                    },
                  ])
                }
              />
            ),
          )}
          {collectionNames.length === 0 && !creating && (
            <button className="sidebar-hint" onClick={() => setCreating(true)}>
              {t("side.firstCollection")}
            </button>
          )}
        </div>

        {tagCounts.size > 0 && (
          <div className={`sidebar-section ${collapsed.includes("tags") ? "sidebar-collapsed" : ""}`}>
            <div className="sidebar-heading sidebar-heading-row">
              <button
                className="sidebar-toggle"
                onClick={() => toggleSection("tags")}
                aria-expanded={!collapsed.includes("tags")}
              >
                <span className="sidebar-chevron">
                  <ChevronDown size={12} strokeWidth={2} />
                </span>
                <span>{t("side.tags")}</span>
              </button>
            </div>
            {!collapsed.includes("tags") &&
              (() => {
                // 按分组聚拢：预置词表分组在前，未分组自定义标签归入「其他」殿后。
                const buckets = new Map<string, [string, number][]>();
                const others: [string, number][] = [];
                for (const entry of tagCounts.entries()) {
                  const gid = groupIdOfTag(entry[0]);
                  if (gid === null) others.push(entry);
                  else {
                    const list = buckets.get(gid) ?? [];
                    list.push(entry);
                    buckets.set(gid, list);
                  }
                }
                const sections = [
                  ...TAG_GROUPS.filter((g) => buckets.has(g.id)).map((g) => ({
                    id: `tags:${g.id}`,
                    label: t(g.labelKey),
                    items: buckets.get(g.id)!,
                  })),
                  ...(others.length > 0
                    ? [{ id: "tags:other", label: t("tag.other"), items: others }]
                    : []),
                ];
                return sections.map((section) => (
                  <div
                    key={section.id}
                    className={`sidebar-taggroup ${collapsed.includes(section.id) ? "sidebar-collapsed" : ""}`}
                  >
                    <button
                      className="sidebar-toggle sidebar-taggroup-toggle"
                      onClick={() => toggleSection(section.id)}
                      aria-expanded={!collapsed.includes(section.id)}
                    >
                      <span className="sidebar-chevron">
                        <ChevronDown size={11} strokeWidth={2} />
                      </span>
                      <span>{section.label}</span>
                    </button>
                    {!collapsed.includes(section.id) &&
                      section.items.map(([tag, count]) => (
                        <NavRow
                          key={tag}
                          nav={{ kind: "tag", tag }}
                          icon={<Tag size={15} strokeWidth={1.5} />}
                          label={tag}
                          count={count}
                          index={i++}
                          onContextMenu={(e) =>
                            openContextMenu(e, [
                              {
                                label: translate("menu.removeTagEverywhere", { tag }),
                                danger: true,
                                action: () => removeTagEverywhere(tag),
                              },
                            ])
                          }
                        />
                      ))}
                  </div>
                ));
              })()}
            {!collapsed.includes("tags") && (
              <button className="sidebar-ai-import" onClick={() => void importAiCatalog()}>
                <Sparkles size={12} strokeWidth={1.7} />
                {t("side.importAiCatalog")}
              </button>
            )}
          </div>
        )}
      </div>

      <div className="sidebar-footer">
        <NavRow
          nav={{ kind: "folders" }}
          icon={<FolderCog size={15} strokeWidth={1.5} />}
          label={t("side.folders")}
          index={i++}
        />
        <NavRow
          nav={{ kind: "trash" }}
          icon={<Trash2 size={15} strokeWidth={1.5} />}
          label={t("side.trash")}
          count={trash.length}
          index={i++}
        />
        <motion.button
          className="nav-row"
          onClick={openToolsMenu}
          initial={{ opacity: 0, x: -12 }}
          animate={{ opacity: 1, x: 0 }}
          transition={{ ...springSoft, delay: staggerDelay(i++) }}
          whileTap={{ scale: 0.97 }}
          aria-haspopup="menu"
        >
          <span className="nav-icon">
            <MoreHorizontal size={15} strokeWidth={1.5} />
          </span>
          <span className="nav-label">{t("side.moreTools")}</span>
          <kbd className="kbd nav-kbd" title={t("side.shortcuts")}>
            <Keyboard size={10} strokeWidth={1.8} />
          </kbd>
        </motion.button>
      </div>
    </motion.aside>
  );
}
