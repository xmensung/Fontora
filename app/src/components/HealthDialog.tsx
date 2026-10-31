import { AnimatePresence, motion } from "motion/react";
import {
  AlertTriangle,
  CheckCircle2,
  Info,
  LoaderCircle,
  Stethoscope,
  X,
  XCircle,
} from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import { springSnappy } from "../design/springs";
import { ipc, type Health, type HealthIssue, type HealthLevel } from "../lib/ipc";
import { useFocusTrap } from "../lib/useFocusTrap";
import { useT } from "../lib/i18n";

interface Props {
  open: boolean;
  onClose: () => void;
  path: string;
  faceIndex: number;
  family: string;
  facesInFile: number;
}

const ICON: Record<HealthLevel, typeof Info> = {
  error: XCircle,
  warn: AlertTriangle,
  info: Info,
};

/** 必备的那几张表，界面上按固定顺序排，缺了的也留位置。 */
const CORE_TABLES = ["head", "hhea", "maxp", "cmap", "name", "OS/2", "post", "glyf", "loca", "CFF ", "CFF2", "fvar"];

function scoreTone(score: number) {
  if (score >= 90) return "good";
  if (score >= 70) return "ok";
  return "bad";
}

function ScoreRing({ score }: { score: number }) {
  const r = 26;
  const c = 2 * Math.PI * r;
  const filled = (Math.max(0, Math.min(100, score)) / 100) * c;
  return (
    <div className={`hp-ring ${scoreTone(score)}`} aria-label={`${score}/100`}>
      <svg viewBox="0 0 64 64" width="64" height="64" aria-hidden="true">
        <circle cx="32" cy="32" r={r} className="hp-ring-track" />
        <circle
          cx="32"
          cy="32"
          r={r}
          className="hp-ring-value"
          strokeDasharray={`${filled} ${c - filled}`}
          transform="rotate(-90 32 32)"
        />
      </svg>
      <span className="hp-ring-num tabular">{score}</span>
    </div>
  );
}

function IssueRow({ issue }: { issue: HealthIssue }) {
  const Icon = ICON[issue.level];
  return (
    <div className={`hp-issue ${issue.level}`}>
      <span className="hp-issue-icon" aria-hidden="true">
        <Icon size={14} strokeWidth={1.8} />
      </span>
      <div className="hp-issue-body">
        <div className="hp-issue-title">{issue.title}</div>
        {issue.detail && <div className="hp-issue-detail">{issue.detail}</div>}
      </div>
    </div>
  );
}

/** 字体体检：结构、命名、字符覆盖逐项过一遍，给出能不能放心用的结论。 */
export function HealthDialog({ open: isOpen, onClose, path, faceIndex, family, facesInFile }: Props) {
  const t = useT();
  const trapRef = useFocusTrap<HTMLDivElement>(isOpen);
  const [health, setHealth] = useState<Health | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!isOpen) return;
    let stale = false;
    setHealth(null);
    setError(null);
    ipc
      .fontHealth(path, faceIndex)
      .then((h) => {
        if (!stale) setHealth(h);
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

  const counts = useMemo(() => {
    const base = { error: 0, warn: 0, info: 0 };
    for (const issue of health?.issues ?? []) base[issue.level] += 1;
    return base;
  }, [health]);

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
            className="hp-panel"
            ref={trapRef}
            role="dialog"
            aria-modal="true"
            aria-label={t("health.title")}
            initial={{ opacity: 0, scale: 0.985, y: 10 }}
            animate={{ opacity: 1, scale: 1, y: 0 }}
            exit={{ opacity: 0, scale: 0.99, y: 6, transition: { duration: 0.12 } }}
            transition={springSnappy}
            onClick={(e) => e.stopPropagation()}
          >
            <header className="cm-head">
              <span className="cm-head-icon" aria-hidden="true">
                <Stethoscope size={15} strokeWidth={1.7} />
              </span>
              <div className="cm-head-text">
                <div className="cm-title">{t("health.title")}</div>
                <div className="cm-sub">
                  {family}
                  {facesInFile > 1 ? ` · ${t("export.inCollection", { count: facesInFile })}` : ""}
                </div>
              </div>
              <button className="cm-close" aria-label={t("export.close")} onClick={onClose}>
                <X size={15} strokeWidth={1.7} />
              </button>
            </header>

            {error && (
              <div className="cm-error">
                <strong>{t("health.failed")}</strong>
                <span className="fd-mono">{error}</span>
              </div>
            )}

            {!error && !health && (
              <div className="cm-loading">
                <LoaderCircle size={16} strokeWidth={1.6} className="fd-spin" />
                {t("health.checking")}
              </div>
            )}

            {health && (
              <div className="hp-body">
                <div className="hp-summary">
                  <ScoreRing score={health.score} />
                  <div className="hp-summary-text">
                    <div className="hp-verdict">
                      {health.score >= 90
                        ? t("health.verdictGood")
                        : health.score >= 70
                          ? t("health.verdictOk")
                          : t("health.verdictBad")}
                    </div>
                    <div className="hp-tally">
                      {counts.error > 0 && (
                        <span className="hp-tally-item error">
                          {t("health.tallyError", { count: counts.error })}
                        </span>
                      )}
                      {counts.warn > 0 && (
                        <span className="hp-tally-item warn">
                          {t("health.tallyWarn", { count: counts.warn })}
                        </span>
                      )}
                      {counts.info > 0 && (
                        <span className="hp-tally-item info">
                          {t("health.tallyInfo", { count: counts.info })}
                        </span>
                      )}
                      {health.issues.length === 0 && (
                        <span className="hp-tally-item ok">
                          <CheckCircle2 size={12} strokeWidth={2} /> {t("health.tallyNone")}
                        </span>
                      )}
                    </div>
                  </div>
                </div>

                {health.issues.length > 0 && (
                  <div className="hp-issues">
                    {health.issues.map((issue, i) => (
                      <IssueRow key={`${issue.title}-${i}`} issue={issue} />
                    ))}
                  </div>
                )}

                <section className="hp-section">
                  <div className="hp-section-head">{t("health.tables")}</div>
                  <div className="hp-tables">
                    {CORE_TABLES.map((tag) => {
                      const row = health.tables.find((x) => x.tag === tag);
                      return (
                        <div className={`hp-table ${row ? "has" : "missing"}`} key={tag}>
                          <span className="hp-table-tag fd-mono">{tag.trim()}</span>
                          <span className="hp-table-len tabular">
                            {row ? `${row.length.toLocaleString()} B` : t("health.tableMissing")}
                          </span>
                        </div>
                      );
                    })}
                  </div>
                  <div className="hp-table-note">
                    {t("health.tableCount", { count: health.tables.length })}
                    {health.tables.some((x) => !x.checksumOk) && ` · ${t("health.checksumNote")}`}
                  </div>
                </section>
              </div>
            )}
          </motion.div>
        </motion.div>
      )}
    </AnimatePresence>
  );
}
