import { motion } from "motion/react";
import { BarChart3, Clock, Eraser, EyeOff, Sparkles, Trash2 } from "lucide-react";
import { useMemo, useState } from "react";
import { spring, staggerDelay } from "../design/springs";
import { formatBytes } from "../lib/fontLoader";
import { useT, type TKey } from "../lib/i18n";
import type { UsageRow } from "../lib/ipc";
import { familiesFor, useFontStore, type Family } from "../state/fontStore";

const DAY = 86400;

/** 「最后露面」：看过 / 装过 / 被引用，取最晚的那次。 */
function lastSeen(row?: UsageRow): number {
  if (!row) return 0;
  return Math.max(row.lastView, row.lastInstall, row.lastRef);
}

function isUsed(row?: UsageRow): boolean {
  return Boolean(row && (row.views > 0 || row.installs > 0 || row.refs > 0));
}

function ago(sec: number, t: (k: TKey, v?: Record<string, string | number>) => string): string {
  if (!sec) return t("usage.never");
  const diff = Math.max(0, Math.floor(Date.now() / 1000) - sec);
  if (diff < 60) return t("usage.justNow");
  if (diff < 3600) return t("usage.minutesAgo", { count: Math.floor(diff / 60) });
  if (diff < DAY) return t("usage.hoursAgo", { count: Math.floor(diff / 3600) });
  if (diff < 30 * DAY) return t("usage.daysAgo", { count: Math.floor(diff / DAY) });
  const d = new Date(sec * 1000);
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`;
}

interface Row {
  fam: Family;
  usage?: UsageRow;
}

function UsageLine({ row, index }: { row: Row; index: number }) {
  const t = useT();
  const openFontDetail = useFontStore((s) => s.openFontDetail);
  const u = row.usage;
  return (
    <motion.button
      className="us-row"
      onClick={() => openFontDetail(row.fam.name)}
      initial={{ opacity: 0, y: 6 }}
      animate={{ opacity: 1, y: 0 }}
      transition={{ ...spring, delay: staggerDelay(index) }}
    >
      <span className="us-name">{row.fam.name}</span>
      <span className="us-counts">
        {u && u.views > 0 && (
          <span className="us-count" title={t("usage.views")}>
            {t("usage.viewShort")} {u.views}
          </span>
        )}
        {u && u.installs > 0 && (
          <span className="us-count" title={t("usage.installs")}>
            {t("usage.installShort")} {u.installs}
          </span>
        )}
        {u && u.refs > 0 && (
          <span className="us-count us-ref" title={t("usage.refs")}>
            {t("usage.refShort")} {u.refs}
          </span>
        )}
      </span>
      <span className="us-when tabular">{ago(lastSeen(u), t)}</span>
    </motion.button>
  );
}

export function UsageView() {
  const t = useT();
  const fonts = useFontStore((s) => s.fonts);
  const tags = useFontStore((s) => s.tags);
  const favorites = useFontStore((s) => s.favorites);
  const collections = useFontStore((s) => s.collections);
  const usage = useFontStore((s) => s.usage);
  const loading = useFontStore((s) => s.usageLoading);
  const createCollection = useFontStore((s) => s.createCollection);
  const addFamiliesToCollection = useFontStore((s) => s.addFamiliesToCollection);
  const clearUsage = useFontStore((s) => s.clearUsage);
  const setNav = useFontStore((s) => s.setNav);

  const [confirmClear, setConfirmClear] = useState(false);
  const [showAllNever, setShowAllNever] = useState(false);

  const families = useMemo(() => [...familiesFor(fonts, tags).values()], [fonts, tags]);

  const used = useMemo(
    () =>
      families
        .map((fam) => ({ fam, usage: usage[fam.name] }))
        .filter((r) => isUsed(r.usage))
        .sort(
          (a, b) => lastSeen(b.usage) - lastSeen(a.usage) || a.fam.name.localeCompare(b.fam.name),
        ),
    [families, usage],
  );

  const never = useMemo(
    () =>
      families
        .map((fam) => ({ fam, usage: usage[fam.name] }))
        .filter((r) => !isUsed(r.usage))
        .sort((a, b) => b.fam.totalSize - a.fam.totalSize || a.fam.name.localeCompare(b.fam.name)),
    [families, usage],
  );

  // 「可以清一清」的口径故意保守：90 天没露面、没收藏、不在任何集合里、且过了 1MB。
  const cleanable = useMemo(() => {
    const keep = new Set<string>();
    for (const members of Object.values(collections)) {
      for (const name of members) keep.add(name);
    }
    const cutoff = Math.floor(Date.now() / 1000) - 90 * DAY;
    return [...used, ...never]
      .filter((r) => lastSeen(r.usage) < cutoff)
      .filter((r) => !favorites.includes(r.fam.name) && !keep.has(r.fam.name))
      .filter((r) => r.fam.totalSize >= 1024 * 1024)
      .sort((a, b) => b.fam.totalSize - a.fam.totalSize);
  }, [used, never, favorites, collections]);

  const recent7 = used.filter(
    (r) => lastSeen(r.usage) >= Math.floor(Date.now() / 1000) - 7 * DAY,
  ).length;
  const neverBytes = never.reduce((sum, r) => sum + r.fam.totalSize, 0);
  const cleanableBytes = cleanable.reduce((sum, r) => sum + r.fam.totalSize, 0);
  const neverShown = showAllNever ? never : never.slice(0, 30);

  const parkThem = async () => {
    if (cleanable.length === 0) return;
    const name = t("usage.parkCollection");
    const names = cleanable.map((r) => r.fam.name);
    if (!collections[name]) {
      await createCollection(name);
    }
    await addFamiliesToCollection(name, names);
    setNav({ kind: "collection", name });
  };

  return (
    <div className="ref-view">
      <div className="ref-inner">
        <header className="ref-head">
          <div className="ref-head-text">
            <h1 className="ref-title">{t("usage.title")}</h1>
            <p className="ref-sub">{t("usage.subtitle")}</p>
          </div>
          <div className="ref-head-actions">
            <button
              className={`ref-btn ${confirmClear ? "ref-btn-danger" : ""}`}
              disabled={loading || used.length === 0}
              onClick={() => {
                if (!confirmClear) {
                  setConfirmClear(true);
                  return;
                }
                setConfirmClear(false);
                void clearUsage();
              }}
              onBlur={() => setConfirmClear(false)}
            >
              <Eraser size={13} strokeWidth={1.7} />
              {confirmClear ? t("usage.clearConfirm") : t("usage.clear")}
            </button>
          </div>
        </header>

        <div className="ref-body">
          <div className="ref-stats">
            <div className="ref-stat">
              <span className="ref-stat-value tabular">{families.length}</span>
              <span className="ref-stat-label">{t("usage.statFamilies")}</span>
            </div>
            <div className="ref-stat primary">
              <span className="ref-stat-value tabular">{used.length}</span>
              <span className="ref-stat-label">{t("usage.statUsed")}</span>
            </div>
            <div className="ref-stat">
              <span className="ref-stat-value tabular">{recent7}</span>
              <span className="ref-stat-label">{t("usage.statWeek")}</span>
            </div>
            <div className="ref-stat">
              <span className="ref-stat-value tabular">{never.length}</span>
              <span className="ref-stat-label">{t("usage.statNever")}</span>
            </div>
            <div className="ref-stat dim">
              <span className="ref-stat-value tabular">{formatBytes(neverBytes)}</span>
              <span className="ref-stat-label">{t("usage.statNeverBytes")}</span>
            </div>
          </div>

          {used.length === 0 && (
            <div className="ref-empty">
              <BarChart3 size={22} strokeWidth={1.4} />
              <p>{t("usage.emptyTitle")}</p>
              <p className="ref-empty-sub">{t("usage.emptySub")}</p>
            </div>
          )}

          {used.length > 0 && (
            <section className="ref-section">
              <div className="ref-section-head">
                <Clock size={13} strokeWidth={1.8} />
                {t("usage.recentTitle")}
              </div>
              <p className="us-hint">{t("usage.recentHint")}</p>
              <div className="us-list">
                {used.slice(0, 14).map((row, i) => (
                  <UsageLine key={row.fam.name} row={row} index={i} />
                ))}
              </div>
            </section>
          )}

          {never.length > 0 && (
            <section className="ref-section">
              <div className="ref-section-head">
                <EyeOff size={13} strokeWidth={1.8} />
                {t("usage.neverTitle", { count: never.length })}
              </div>
              <p className="us-hint">{t("usage.neverHint")}</p>
              <div className="us-list">
                {neverShown.map((row, i) => (
                  <UsageLine key={row.fam.name} row={row} index={i} />
                ))}
              </div>
              {never.length > neverShown.length && (
                <button className="us-more" onClick={() => setShowAllNever(true)}>
                  {t("usage.showAll", { count: never.length })}
                </button>
              )}
            </section>
          )}

          <section className="ref-section">
            <div className="ref-section-head">
              <Sparkles size={13} strokeWidth={1.8} />
              {t("usage.cleanTitle")}
            </div>
            <p className="us-hint">{t("usage.cleanHint")}</p>
            {cleanable.length === 0 ? (
              <p className="ref-none">{t("usage.cleanNone")}</p>
            ) : (
              <>
                <div className="us-clean">
                  <div className="us-clean-text">
                    <strong className="tabular">{cleanable.length}</strong>
                    {t("usage.cleanCount")}
                    <span className="us-clean-bytes tabular">{formatBytes(cleanableBytes)}</span>
                  </div>
                  <button className="ref-btn" onClick={() => void parkThem()}>
                    <Trash2 size={13} strokeWidth={1.7} />
                    {t("usage.cleanAction", { name: t("usage.parkCollection") })}
                  </button>
                </div>
                <p className="us-hint dim">{t("usage.cleanHintSub")}</p>
              </>
            )}
          </section>
        </div>
      </div>
    </div>
  );
}
