import { AnimatePresence, motion } from "motion/react";
import { AlertTriangle, Check, Copy, HardDrive, RefreshCw, ShieldCheck, Trash2 } from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import { spring, staggerDelay } from "../design/springs";
import { formatBytes } from "../lib/fontLoader";
import { useT } from "../lib/i18n";
import type { DupeFile, DupeGroup } from "../lib/ipc";
import { useFontStore } from "../state/fontStore";

function formatDate(seconds: number): string {
  if (!seconds) return "—";
  const d = new Date(seconds * 1000);
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(
    d.getDate(),
  ).padStart(2, "0")}`;
}

/** 默认勾选：只有「字节完全相同且可删」的非保留项才预选。 */
function defaultChecked(groups: DupeGroup[]): string[] {
  const out: string[] = [];
  for (const g of groups) {
    if (g.kind !== "identical") continue;
    for (const f of g.files) {
      if (f.deletable && !f.kept) out.push(f.path);
    }
  }
  return out;
}

function FileRow({
  file,
  checked,
  onToggle,
  identical,
}: {
  file: DupeFile;
  checked: boolean;
  onToggle: () => void;
  identical: boolean;
}) {
  const t = useT();
  return (
    <label
      className={`dupe-row ${file.deletable ? "" : "dupe-row-locked"} ${checked ? "dupe-row-on" : ""}`}
    >
      <span className="dupe-check">
        <input
          type="checkbox"
          checked={checked}
          disabled={!file.deletable}
          onChange={onToggle}
        />
        <span className="dupe-check-mark" aria-hidden="true">
          {file.deletable ? <Check size={11} strokeWidth={3} /> : <ShieldCheck size={11} strokeWidth={2} />}
        </span>
      </span>

      <span className="dupe-row-main">
        <span className="dupe-row-head">
          <span className="dupe-file tabular">{file.fileName}</span>
          {file.kept && <span className="dupe-tag dupe-tag-keep">{t("dupe.keep")}</span>}
          {file.system ? (
            <span className="dupe-tag">{t("dupe.system")}</span>
          ) : (
            <span className="dupe-tag dupe-tag-user">{t("dupe.userFont")}</span>
          )}
          {identical && <span className="dupe-tag dupe-tag-same">{t("dupe.sameBytes")}</span>}
        </span>
        <span className="dupe-path fd-mono" title={file.path}>
          {file.path}
        </span>
      </span>

      <span className="dupe-meta tabular">
        <span>{formatBytes(file.size)}</span>
        <span className="dupe-meta-dim">{formatDate(file.modified)}</span>
      </span>
    </label>
  );
}

function GroupCard({
  group,
  checked,
  setChecked,
  index,
}: {
  group: DupeGroup;
  checked: Set<string>;
  setChecked: (next: Set<string>) => void;
  index: number;
}) {
  const t = useT();
  const identical = group.kind === "identical";

  const toggle = (path: string) => {
    const next = new Set(checked);
    if (next.has(path)) next.delete(path);
    else next.add(path);
    setChecked(next);
  };

  return (
    <motion.section
      className="dupe-group"
      initial={{ opacity: 0, y: 12 }}
      animate={{ opacity: 1, y: 0 }}
      transition={{ ...spring, delay: staggerDelay(index) }}
    >
      <header className="dupe-group-head">
        <div className="dupe-group-title">
          <span className={`dupe-kind ${identical ? "dupe-kind-same" : "dupe-kind-name"}`}>
            {identical ? <Copy size={11} strokeWidth={2.2} /> : <AlertTriangle size={11} strokeWidth={2.2} />}
            {identical ? t("dupe.kindIdentical") : t("dupe.kindSameName")}
          </span>
          <h3>{group.label || t("dupe.untitled")}</h3>
        </div>
        <div className="dupe-group-side">
          <span className="dupe-count tabular">{t("dupe.fileCount", { count: group.files.length })}</span>
          <span className="dupe-reclaim tabular">
            {group.reclaimable > 0
              ? t("dupe.reclaimable", { size: formatBytes(group.reclaimable) })
              : t("dupe.nothingToFree")}
          </span>
        </div>
      </header>

      <p className="dupe-group-note">
        {identical ? t("dupe.noteIdentical") : t("dupe.noteSameName")}
      </p>

      <div className="dupe-rows">
        {group.files.map((f) => (
          <FileRow
            key={f.path}
            file={f}
            identical={identical}
            checked={checked.has(f.path)}
            onToggle={() => toggle(f.path)}
          />
        ))}
      </div>
    </motion.section>
  );
}

export function DuplicatesView() {
  const t = useT();
  const groups = useFontStore((s) => s.dupeGroups);
  const loading = useFontStore((s) => s.dupeLoading);
  const loadDuplicates = useFontStore((s) => s.loadDuplicates);
  const deleteDuplicates = useFontStore((s) => s.deleteDuplicates);

  const [checked, setChecked] = useState<Set<string>>(new Set<string>());
  const [loadedOnce, setLoadedOnce] = useState(false);

  useEffect(() => {
    void loadDuplicates().finally(() => setLoadedOnce(true));
  }, [loadDuplicates]);

  // 第一次拿到结果时套用默认勾选；之后刷新则保留用户的选择。
  useEffect(() => {
    if (!loadedOnce) return;
    setChecked((prev) => (prev.size === 0 ? new Set(defaultChecked(groups)) : prev));
  }, [groups, loadedOnce]);

  const allFiles = useMemo(() => groups.flatMap((g) => g.files), [groups]);
  const chosen = useMemo(
    () => allFiles.filter((f) => checked.has(f.path) && f.deletable),
    [allFiles, checked],
  );
  const chosenSize = chosen.reduce((sum, f) => sum + f.size, 0);
  const totalReclaim = groups.reduce((sum, g) => sum + g.reclaimable, 0);
  const locked = allFiles.filter((f) => !f.deletable).length;

  const busy = loading && !loadedOnce;

  if (busy) {
    return (
      <div className="dupe-view">
        <div className="dupe-bar">
          <RefreshCw size={14} strokeWidth={1.8} className="fd-spin" />
          <span>{t("dupe.scanning")}</span>
        </div>
      </div>
    );
  }

  if (groups.length === 0) {
    return (
      <motion.div
        className="empty-state"
        initial={{ opacity: 0, y: 16 }}
        animate={{ opacity: 1, y: 0 }}
        transition={spring}
      >
        <span className="empty-tile">
          <Check size={26} strokeWidth={1.5} />
        </span>
        <h2>{t("dupe.emptyTitle")}</h2>
        <p>{t("dupe.emptyBody")}</p>
        <button className="dupe-rescan" onClick={() => void loadDuplicates()}>
          <RefreshCw size={13} strokeWidth={1.8} />
          {t("dupe.rescan")}
        </button>
      </motion.div>
    );
  }

  return (
    <div className="dupe-view">
      <div className="dupe-bar">
        <span className="dupe-summary">
          {t("dupe.summary", { groups: groups.length, files: allFiles.length })}
          {totalReclaim > 0 && (
            <>
              <span className="dupe-dot" aria-hidden="true" />
              {t("dupe.totalReclaim", { size: formatBytes(totalReclaim) })}
            </>
          )}
          {locked > 0 && (
            <>
              <span className="dupe-dot" aria-hidden="true" />
              {t("dupe.lockedCount", { count: locked })}
            </>
          )}
        </span>
        <button className="dupe-rescan" onClick={() => void loadDuplicates()} disabled={loading}>
          <RefreshCw size={13} strokeWidth={1.8} className={loading ? "fd-spin" : undefined} />
          {t("dupe.rescan")}
        </button>
      </div>

      <div className="dupe-list">
        {groups.map((g, i) => (
          <GroupCard key={g.id} group={g} checked={checked} setChecked={setChecked} index={i} />
        ))}
      </div>

      <p className="dupe-footnote">
        <HardDrive size={12} strokeWidth={1.7} />
        {t("dupe.footnote")}
      </p>

      <AnimatePresence>
        {chosen.length > 0 && (
          <motion.div
            className="dupe-actionbar glass-e3"
            initial={{ y: 24, opacity: 0 }}
            animate={{ y: 0, opacity: 1 }}
            exit={{ y: 20, opacity: 0 }}
            transition={spring}
          >
            <span className="dupe-action-text">
              {t("dupe.selected", { count: chosen.length })}
              <span className="dupe-dot" aria-hidden="true" />
              <span className="tabular">{formatBytes(chosenSize)}</span>
            </span>
            <button
              className="dupe-action-cancel"
              onClick={() => setChecked(new Set<string>())}
            >
              {t("dupe.clearSelection")}
            </button>
            <motion.button
              className="dupe-action-run"
              whileHover={{ y: -1 }}
              whileTap={{ scale: 0.97 }}
              onClick={() => {
                setChecked(new Set<string>());
                void deleteDuplicates(chosen.map((f) => f.path));
              }}
            >
              <Trash2 size={13} strokeWidth={1.8} />
              {t("dupe.moveToTrash")}
            </motion.button>
          </motion.div>
        )}
      </AnimatePresence>
    </div>
  );
}
