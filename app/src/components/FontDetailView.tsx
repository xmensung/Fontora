import { openUrl, revealItemInDir } from "@tauri-apps/plugin-opener";
import { AnimatePresence, motion } from "motion/react";
import { ArrowLeft, Check, Copy, Download, ExternalLink, FolderOpen, Grid3x3, LoaderCircle, Minus, Package, Star, Stethoscope, X } from "lucide-react";
import { useEffect, useMemo, useRef, useState } from "react";
import { CharMapDialog } from "./CharMapDialog";
import { ExportDialog } from "./ExportDialog";
import { HealthDialog } from "./HealthDialog";
import { PillToggle } from "../design/primitives/PillToggle";
import { spring } from "../design/springs";
import { toast } from "../design/primitives/Toast";
import { playStar } from "../lib/sound";
import { useFontCss, formatBytes, pathBasename } from "../lib/fontLoader";
import { ipc, type CoverageRow, type FontDetail } from "../lib/ipc";
import { useFocusTrap } from "../lib/useFocusTrap";
import { useT } from "../lib/i18n";
import { familiesFor, useFontStore } from "../state/fontStore";

const GROUPS = [
  { key: "simp", labelKey: "fontDetail.group.simp" },
  { key: "both", labelKey: "fontDetail.group.both" },
  { key: "trad", labelKey: "fontDetail.group.trad" },
] as const;

function Stars({ value }: { value: number | null }) {
  if (value === null) return null;
  return (
    <span className="fd-stars" aria-label={`${value}/5`}>
      {Array.from({ length: 5 }, (_, i) => (
        <span key={i} className={i < value ? "on" : "off"}>
          ★
        </span>
      ))}
    </span>
  );
}

function CoverageItem({ row, graded }: { row: CoverageRow; graded: boolean }) {
  const pct = row.total > 0 ? row.covered / row.total : 0;
  return (
    <div className="fd-cov-row">
      <span className="fd-cov-label" title={row.label}>
        {row.label}
      </span>
      {graded && <Stars value={row.stars} />}
      <span className="fd-cov-count tabular">
        {row.covered}
        <span className="fd-cov-total"> / {row.total}</span>
      </span>
      <span className="fd-cov-bar" aria-hidden="true">
        <span style={{ width: `${Math.max(pct * 100, row.covered > 0 ? 2 : 0)}%` }} />
      </span>
    </div>
  );
}

function CopyRow({ label, value, mono }: { label: string; value: string; mono?: boolean }) {
  const t = useT();
  const copy = () => {
    void navigator.clipboard.writeText(value).then(
      () => toast.success(t("fontDetail.copied"), value.slice(0, 60)),
      () => toast.error(t("fontDetail.copyFailed"), value.slice(0, 60)),
    );
  };
  return (
    <>
      <dt>{label}</dt>
      <dd className={`fd-dd ${mono ? "fd-mono" : ""}`}>
        <span className="fd-dd-text" title={value}>
          {value}
        </span>
        <button className="fd-copy" aria-label={t("fontDetail.copy")} onClick={copy}>
          <Copy size={11} strokeWidth={1.6} />
        </button>
      </dd>
    </>
  );
}

function IntroSection({ detail }: { detail: FontDetail }) {
  const t = useT();
  const [showSpecs, setShowSpecs] = useState(false);
  return (
    <section className="fd-section">
      <header className="fd-section-head">
        <span className="fd-section-index tabular">01</span>
        <h2>{t("fontDetail.section.intro")}</h2>
        <span className="fd-section-rule" aria-hidden="true" />
      </header>

      <div className="fd-intro">
        {detail.intro.map((b, i) => (
          <div className={`fd-intro-block ${i === 0 ? "lead" : ""}`} key={b.label}>
            <div className="fd-intro-label">{b.label}</div>
            <p className="fd-intro-body">{b.body}</p>
          </div>
        ))}
        {detail.description && (
          <div className="fd-intro-block">
            <div className="fd-intro-label">{t("fontDetail.description")}</div>
            <p className="fd-intro-body">{detail.description}</p>
          </div>
        )}
      </div>

      <div className="fd-specs">
        <button
          className="fd-fold"
          aria-expanded={showSpecs}
          onClick={() => setShowSpecs((v) => !v)}
        >
          {t("fontDetail.specs")} · {showSpecs ? t("fontDetail.collapse") : t("fontDetail.expand")}
        </button>
        <AnimatePresence initial={false}>
          {showSpecs && (
            <motion.div
              className="fd-specs-body"
              initial={{ height: 0, opacity: 0 }}
              animate={{ height: "auto", opacity: 1 }}
              exit={{ height: 0, opacity: 0 }}
              transition={{ duration: 0.18 }}
            >
              <dl className="fd-grid">
        <CopyRow label={t("fontDetail.fileName")} value={pathBasename(detail.path)} />
        <CopyRow label={t("fontDetail.format")} value={detail.format.toUpperCase()} />
        <CopyRow label={t("fontDetail.weight")} value={String(detail.weight)} mono />
        <CopyRow
          label={t("fontDetail.glyphs")}
          value={detail.glyphCount.toLocaleString()}
          mono
        />
        <CopyRow label={t("fontDetail.fileSize")} value={formatBytes(detail.fileSize)} mono />
        {detail.postscriptName && (
          <CopyRow label={t("fontDetail.postscript")} value={detail.postscriptName} mono />
        )}
        {detail.version && <CopyRow label={t("fontDetail.version")} value={detail.version} />}
        {detail.created && (
          <CopyRow label={t("fontDetail.created")} value={detail.created} mono />
        )}
        {detail.modifiedDate && (
          <CopyRow label={t("fontDetail.modified")} value={detail.modifiedDate} mono />
        )}
        {detail.sha256 && (
          <CopyRow label={t("fontDetail.hash")} value={detail.sha256} mono />
        )}
                <dt>{t("fontDetail.path")}</dt>
                <dd className="fd-dd fd-mono">
                  <span className="fd-dd-text" title={detail.path}>
                    {detail.path}
                  </span>
                  <button
                    className="fd-copy"
                    aria-label={t("fontDetail.reveal")}
                    onClick={() => void revealItemInDir(detail.path).catch(() => {})}
                  >
                    <FolderOpen size={11} strokeWidth={1.6} />
                  </button>
                </dd>
              </dl>

              {detail.names.length > 1 && (
                <div className="fd-names">
                  <div className="fd-intro-label">{t("fontDetail.names")}</div>
                  <div className="fd-name-chips">
                    {detail.names.map((n) => (
                      <span className="fd-name-chip" key={n.langCode}>
                        <span className="fd-name-lang">{n.lang}</span>
                        {n.family ?? n.style}
                      </span>
                    ))}
                  </div>
                </div>
              )}
            </motion.div>
          )}
        </AnimatePresence>
      </div>
    </section>
  );
}

function LicenseSection({ detail }: { detail: FontDetail }) {
  const t = useT();
  const lic = detail.license;
  const [showText, setShowText] = useState(false);
  const hasText = Boolean(lic.licenseText && lic.licenseText.trim().length > 40);

  return (
    <section className="fd-section">
      <header className="fd-section-head">
        <span className="fd-section-index tabular">02</span>
        <h2>{t("fontDetail.section.license")}</h2>
        <span className="fd-section-rule" aria-hidden="true" />
      </header>

      <div className="fd-verdict">
        <div className="fd-verdict-main">
          <span className="fd-verdict-label">{t("fontDetail.embedding")}</span>
          <span className="fd-verdict-value">{lic.headline}</span>
        </div>
        <span className="fd-verdict-code tabular">fsType 0x{lic.fsType.toString(16).padStart(4, "0")}</span>
      </div>

      <div className="fd-flags">
        {lic.flags.map((f) => (
          <div className={`fd-flag ${f.on ? "on" : "off"}`} key={f.bit}>
            <span className="fd-flag-mark" aria-hidden="true">
              {f.on ? <Check size={11} strokeWidth={2.4} /> : <Minus size={11} strokeWidth={2.4} />}
            </span>
            <span className="fd-flag-body">
              <span className="fd-flag-label">{f.label}</span>
              <span className="fd-flag-detail">{f.detail}</span>
            </span>
          </div>
        ))}
        <div className={`fd-flag ${lic.fsType === 0 ? "on" : "off"}`}>
          <span className="fd-flag-mark" aria-hidden="true">
            {lic.fsType === 0 ? (
              <Check size={11} strokeWidth={2.4} />
            ) : (
              <Minus size={11} strokeWidth={2.4} />
            )}
          </span>
          <span className="fd-flag-body">
            <span className="fd-flag-label">{t("fontDetail.installable")}</span>
            <span className="fd-flag-detail">{t("fontDetail.installableDetail")}</span>
          </span>
        </div>
      </div>

      <div className="fd-lists">
        <div className="fd-list fd-list-allow">
          <div className="fd-list-head">
            <Check size={12} strokeWidth={2.2} /> {t("fontDetail.allow")}
          </div>
          <ul>
            {lic.allow.map((s) => (
              <li key={s}>{s}</li>
            ))}
          </ul>
        </div>
        <div className="fd-list fd-list-deny">
          <div className="fd-list-head">
            <X size={12} strokeWidth={2.2} /> {t("fontDetail.deny")}
          </div>
          <ul>
            {lic.deny.map((s) => (
              <li key={s}>{s}</li>
            ))}
          </ul>
        </div>
      </div>

      <div className="fd-license-fields">
        {lic.copyright && (
          <div className="fd-field">
            <div className="fd-intro-label">{t("fontDetail.copyright")}</div>
            <p className="fd-field-body">{lic.copyright}</p>
          </div>
        )}
        {lic.trademark && (
          <div className="fd-field">
            <div className="fd-intro-label">{t("fontDetail.trademark")}</div>
            <p className="fd-field-body">{lic.trademark}</p>
          </div>
        )}
        <div className="fd-links">
          {lic.licenseUrl && (
            <button className="fd-link" onClick={() => void openUrl(lic.licenseUrl!).catch(() => {})}>
              <ExternalLink size={12} strokeWidth={1.6} /> {t("fontDetail.licenseUrl")}
            </button>
          )}
          {lic.vendorUrl && (
            <button className="fd-link" onClick={() => void openUrl(lic.vendorUrl!).catch(() => {})}>
              <ExternalLink size={12} strokeWidth={1.6} /> {t("fontDetail.vendorUrl")}
            </button>
          )}
        </div>
        {hasText && (
          <div className="fd-field">
            <button className="fd-fold" onClick={() => setShowText((v) => !v)}>
              {showText ? t("fontDetail.hideLicenseText") : t("fontDetail.showLicenseText")}
            </button>
            <AnimatePresence initial={false}>
              {showText && (
                <motion.pre
                  className="fd-license-text"
                  initial={{ height: 0, opacity: 0 }}
                  animate={{ height: "auto", opacity: 1 }}
                  exit={{ height: 0, opacity: 0 }}
                  transition={{ duration: 0.18 }}
                >
                  {lic.licenseText}
                </motion.pre>
              )}
            </AnimatePresence>
          </div>
        )}
      </div>

      <p className="fd-note">{t("fontDetail.licenseNote")}</p>
    </section>
  );
}

function CoverageSection({ detail }: { detail: FontDetail }) {
  const t = useT();
  const listed = useMemo(() => {
    const byGroup = new Map<string, CoverageRow[]>();
    for (const row of detail.coverage.listed) {
      const arr = byGroup.get(row.group) ?? [];
      arr.push(row);
      byGroup.set(row.group, arr);
    }
    return byGroup;
  }, [detail]);

  return (
    <section className="fd-section">
      <header className="fd-section-head">
        <span className="fd-section-index tabular">03</span>
        <h2>{t("fontDetail.section.coverage")}</h2>
        <span className="fd-section-rule" aria-hidden="true" />
      </header>

      <div className="fd-stat-chips">
        {[
          { label: t("fontDetail.han"), value: detail.cjk.han, primary: true },
          { label: t("fontDetail.totalChars"), value: detail.cjk.totalChars, primary: true },
          { label: t("fontDetail.hanBasic"), value: detail.cjk.hanBasic },
          { label: t("fontDetail.hanExt"), value: detail.cjk.hanExtA + detail.cjk.hanExtB + detail.cjk.hanExtC + detail.cjk.hanExtD + detail.cjk.hanExtE + detail.cjk.hanExtF + detail.cjk.hanExtG + detail.cjk.hanExtH + detail.cjk.hanExtI },
          { label: t("fontDetail.kana"), value: detail.cjk.kana },
          { label: t("fontDetail.latin"), value: detail.cjk.latin },
        ].map((c) => (
          <div className={`fd-stat ${c.primary ? "primary" : ""}`} key={c.label}>
            <span className="fd-stat-value tabular">{c.value.toLocaleString()}</span>
            <span className="fd-stat-label">{c.label}</span>
          </div>
        ))}
      </div>

      <p className="fd-section-note">{t("fontDetail.coverageNote")}</p>

      <div className="fd-cov-columns">
        <div className="fd-cov-col">
          <div className="fd-col-head">{t("fontDetail.listed")}</div>
          {GROUPS.map((g) => {
            const rows = listed.get(g.key) ?? [];
            if (rows.length === 0) return null;
            return (
              <div className="fd-cov-group" key={g.key}>
                <div className="fd-cov-group-head">{t(g.labelKey)}</div>
                {rows.map((row) => (
                  <CoverageItem key={row.id} row={row} graded />
                ))}
              </div>
            );
          })}
        </div>
        <div className="fd-cov-col">
          <div className="fd-col-head">{t("fontDetail.blocks")}</div>
          <div className="fd-cov-group">
            {detail.coverage.blocks.map((row) => (
              <CoverageItem key={row.id} row={row} graded={false} />
            ))}
          </div>
        </div>
      </div>
    </section>
  );
}

function SamplesSection({ detail }: { detail: FontDetail }) {
  const t = useT();
  const sampleText = useFontStore((s) => s.sampleText);
  const fonts = useFontStore((s) => s.fonts);
  const tags = useFontStore((s) => s.tags);
  const familyName = useFontStore((s) => s.fontDetailFamily);
  const family = familyName ? familiesFor(fonts, tags).get(familyName) ?? null : null;

  const [styleId, setStyleId] = useState<string | null>(null);
  const [scale, setScale] = useState(1);

  const lead =
    family?.faces.find((f) => f.id === styleId) ??
    family?.faces.find((f) => f.style === detail.style) ??
    family?.faces[0] ??
    null;
  const { fontFamily } = useFontCss(lead);

  const faceStyle = lead?.italic ? "italic" : "normal";

  return (
    <section className="fd-section">
      <header className="fd-section-head">
        <span className="fd-section-index tabular">04</span>
        <h2>{t("fontDetail.section.samples")}</h2>
        <span className="fd-section-rule" aria-hidden="true" />
        <div className="fd-sample-tools">
          {family && family.faces.length > 1 && (
            <div className="fd-style-chips">
              {family.faces.slice(0, 8).map((f) => (
                <button
                  key={f.id}
                  className={`fd-style-chip ${lead?.id === f.id ? "on" : ""}`}
                  onClick={() => setStyleId(f.id)}
                >
                  {f.style}
                </button>
              ))}
            </div>
          )}
          <label className="fd-size">
            <span>{t("fontDetail.size")}</span>
            <input
              type="range"
              min={70}
              max={160}
              value={Math.round(scale * 100)}
              onChange={(e) => setScale(Number(e.target.value) / 100)}
            />
            <span className="tabular">{Math.round(scale * 100)}%</span>
          </label>
        </div>
      </header>

      {!fontFamily && (
        <p className="fd-section-note">{t("fontDetail.sampleLoading")}</p>
      )}

      <div className="fd-samples" style={{ fontFamily: fontFamily ?? undefined, fontStyle: faceStyle }}>
        {sampleText.trim() && (
          <div className="fd-sample">
            <div className="fd-sample-label">{t("fontDetail.customText")}</div>
            <div className="fd-sample-text" style={{ fontSize: 30 * scale }}>
              {sampleText}
            </div>
          </div>
        )}
        {detail.samples.map((s) => (
          <div className="fd-sample" key={s.label}>
            <div className="fd-sample-label">
              {s.label} · <span className="tabular">{Math.round(s.size * scale)}px</span>
            </div>
            <div className="fd-sample-text" style={{ fontSize: s.size * scale }}>
              {s.text}
            </div>
          </div>
        ))}
      </div>
    </section>
  );
}

export function FontDetailView() {
  const t = useT();
  const familyName = useFontStore((s) => s.fontDetailFamily);
  const close = useFontStore((s) => s.closeFontDetail);
  const fonts = useFontStore((s) => s.fonts);
  const tags = useFontStore((s) => s.tags);
  const favorites = useFontStore((s) => s.favorites);
  const toggleFavorite = useFontStore((s) => s.toggleFavorite);
  const setFamilyActive = useFontStore((s) => s.setFamilyActive);
  const ratings = useFontStore((s) => s.ratings);
  const setRating = useFontStore((s) => s.setRating);
  const aliasesByName = useFontStore((s) => s.aliases);
  const openAlias = useFontStore((s) => s.openAlias);
  const trapRef = useFocusTrap<HTMLDivElement>(familyName !== null);
  const bodyRef = useRef<HTMLDivElement>(null);

  const family = familyName ? familiesFor(fonts, tags).get(familyName) ?? null : null;
  const lead =
    family?.faces.find((f) => f.style === "Regular") ?? family?.faces[0] ?? null;
  const favorite = family ? favorites.includes(family.name) : false;
  const rating = family ? (ratings[family.name] ?? 0) : 0;
  const aliases = family ? (aliasesByName[family.name] ?? []) : [];
  // 大标题直接用这款字体渲染，一眼能看出字形风格（加载失败时自动回落界面字体）。
  const { fontFamily: titleFont } = useFontCss(lead);

  const [detail, setDetail] = useState<FontDetail | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [exportOpen, setExportOpen] = useState(false);
  const [charMapOpen, setCharMapOpen] = useState(false);
  const [healthOpen, setHealthOpen] = useState(false);
  // 导出面板挂在 App 根上（P7），开着的时候同样不能让 Esc 关掉整页。
  const exportScopeOpen = useFontStore((s) => s.exportScope !== null);
  const overlayOpen = exportOpen || charMapOpen || healthOpen || exportScopeOpen;

  useEffect(() => {
    if (!lead) return;
    let stale = false;
    setDetail(null);
    setError(null);
    ipc
      .fontDetail(lead.path, lead.faceIndex)
      .then((d) => {
        if (!stale) setDetail(d);
      })
      .catch((e) => {
        if (!stale) setError(String(e));
      });
    return () => {
      stale = true;
    };
  }, [lead?.path, lead?.faceIndex, familyName]);

  // 换字体时把页面滚回顶部，保持读起来像「新的一页」。
  useEffect(() => {
    bodyRef.current?.scrollTo({ top: 0 });
  }, [familyName]);

  useEffect(() => {
    if (!familyName) return;
    const onKey = (e: KeyboardEvent) => {
      // 导出面板 / 字符地图 / 体检开着时，Esc 先关面板，不要把整页关掉。
      if (overlayOpen) return;
      if (e.key === "Escape") {
        e.stopPropagation();
        close();
      }
    };
    window.addEventListener("keydown", onKey, true);
    return () => window.removeEventListener("keydown", onKey, true);
  }, [familyName, close, overlayOpen]);

  return (
    <AnimatePresence>
      {family && (
        <motion.div
          className="fd-page"
          ref={trapRef}
          role="dialog"
          aria-modal="true"
          aria-label={family.name}
          initial={{ opacity: 0, y: 12 }}
          animate={{ opacity: 1, y: 0 }}
          exit={{ opacity: 0, y: 8, transition: { duration: 0.14 } }}
          transition={spring}
        >
          <header className="fd-page-head">
            <button
              className="fd-back"
              onClick={close}
              aria-label={t("fontDetail.back")}
              title={t("fontDetail.back")}
            >
              <ArrowLeft size={15} strokeWidth={1.7} />
              <span>{t("fontDetail.back")}</span>
            </button>
            <span className="fd-page-sep" aria-hidden="true" />
            <div className="fd-page-id">
              <span className="fd-badge">{detail?.format.toUpperCase() ?? "—"}</span>
              <span className="fd-file fd-mono" title={detail?.path ?? family.name}>
                {detail ? pathBasename(detail.path) : family.name}
              </span>
            </div>
            {family && (
              <div className="fd-actions">
                <button
                  className={`fd-icon-btn ${favorite ? "on" : ""}`}
                  aria-pressed={favorite}
                  aria-label={t(favorite ? "detail.unfavorite" : "detail.favorite")}
                  title={t(favorite ? "detail.unfavorite" : "detail.favorite")}
                  onClick={() => {
                    playStar(!favorite);
                    void toggleFavorite(family.name);
                  }}
                >
                  <Star size={15} strokeWidth={1.6} />
                </button>
                <PillToggle
                  on={family.active}
                  disabled={!family.deactivatable}
                  onChange={(on) => void setFamilyActive(family.name, on)}
                  label={t(family.active ? "card.deactivate" : "card.activate", {
                    name: family.name,
                  })}
                />
                <button
                  className="fd-icon-btn"
                  aria-label={t("cmap.open")}
                  title={t("cmap.open")}
                  onClick={() => setCharMapOpen(true)}
                  disabled={!detail}
                >
                  <Grid3x3 size={15} strokeWidth={1.6} />
                </button>
                <button
                  className="fd-icon-btn"
                  aria-label={t("health.open")}
                  title={t("health.open")}
                  onClick={() => setHealthOpen(true)}
                  disabled={!detail}
                >
                  <Stethoscope size={15} strokeWidth={1.6} />
                </button>
                <button
                  className="fd-icon-btn"
                  aria-label={t("export2.open")}
                  title={t("export2.open")}
                  onClick={() => useFontStore.getState().openExport([family.name], family.name)}
                >
                  <Package size={15} strokeWidth={1.6} />
                </button>
                <button
                  className="fd-icon-btn"
                  aria-label={t("export.open")}
                  title={t("export.open")}
                  onClick={() => setExportOpen(true)}
                  disabled={!detail}
                >
                  <Download size={15} strokeWidth={1.6} />
                </button>
                <button
                  className="fd-icon-btn"
                  aria-label={t("fontDetail.reveal")}
                  title={t("fontDetail.reveal")}
                  onClick={() => {
                    if (lead) void revealItemInDir(lead.path).catch(() => {});
                  }}
                >
                  <FolderOpen size={15} strokeWidth={1.6} />
                </button>
              </div>
            )}
          </header>

          <div className="fd-page-body" ref={bodyRef}>
            <div className="fd-page-inner">
              <div className="fd-hero">
                <h1
                  className="fd-hero-title"
                  style={titleFont ? { fontFamily: titleFont } : undefined}
                >
                  {family.name}
                </h1>
                <div className="fd-hero-tags">
                  <div
                    className="fd-rate"
                    role="group"
                    aria-label={t("rating.label")}
                  >
                    {[1, 2, 3, 4, 5].map((n) => (
                      <button
                        key={n}
                        className={`fd-rate-star ${n <= rating ? "on" : ""}`}
                        aria-label={
                          rating === n
                            ? t("rating.clear", { value: n })
                            : t("rating.setTo", { value: n })
                        }
                        title={
                          rating === n
                            ? t("rating.clear", { value: n })
                            : t("rating.setTo", { value: n })
                        }
                        onClick={() => family && setRating(family.name, rating === n ? 0 : n)}
                      >
                        ★
                      </button>
                    ))}
                    {rating > 0 && (
                      <span className="fd-rate-note">{t("rating.short", { value: rating })}</span>
                    )}
                  </div>
                  {aliases.map((alias) => (
                    <span className="fd-alias" key={alias}>
                      {alias}
                    </span>
                  ))}
                  <button
                    className="fd-alias-add"
                    onClick={() => family && openAlias(family.name)}
                    title={t("alias.open")}
                  >
                    + {t("alias.short")}
                  </button>
                </div>
                <div className="fd-hero-meta">
                  {detail ? (
                    <>
                      <span className="fd-hero-chip">{detail.classification}</span>
                      <span>{detail.style}</span>
                      <span className="tabular">
                        {t("fontDetail.glyphShort", {
                          count: detail.glyphCount.toLocaleString(),
                        })}
                      </span>
                      <span className="tabular">{formatBytes(detail.fileSize)}</span>
                      {detail.isVariable && <span>{t("fontDetail.variable")}</span>}
                    </>
                  ) : (
                    <span>{t("fontDetail.loading")}</span>
                  )}
                </div>
              </div>
              {error && (
                <div className="fd-error">
                  <strong>{t("fontDetail.error")}</strong>
                  <span className="fd-mono">{error}</span>
                </div>
              )}
              {!error && !detail && (
                <div className="fd-loading">
                  <LoaderCircle size={16} strokeWidth={1.6} className="fd-spin" />
                  {t("fontDetail.loading")}
                </div>
              )}
              {detail && (
                <motion.div
                  className="fd-sections"
                  initial="hidden"
                  animate="show"
                  variants={{ show: { transition: { staggerChildren: 0.05, delayChildren: 0.04 } } }}
                >
                  {[
                    <IntroSection key="intro" detail={detail} />,
                    <LicenseSection key="license" detail={detail} />,
                    <CoverageSection key="coverage" detail={detail} />,
                    <SamplesSection key="samples" detail={detail} />,
                  ].map((section, i) => (
                    <motion.div
                      key={i}
                      variants={{
                        hidden: { opacity: 0, y: 12 },
                        show: { opacity: 1, y: 0, transition: spring },
                      }}
                    >
                      {section}
                    </motion.div>
                  ))}
                </motion.div>
              )}
            </div>
          </div>

          {detail && lead && (
            <ExportDialog
              open={exportOpen}
              onClose={() => setExportOpen(false)}
              path={lead.path}
              faceIndex={lead.faceIndex}
              family={family.name}
              style={lead.style}
              format={detail.format}
              facesInFile={detail.facesInFile}
            />
          )}

          {detail && lead && (
            <CharMapDialog
              open={charMapOpen}
              onClose={() => setCharMapOpen(false)}
              path={lead.path}
              faceIndex={lead.faceIndex}
              family={family.name}
              fontFamily={titleFont}
            />
          )}

          {detail && lead && (
            <HealthDialog
              open={healthOpen}
              onClose={() => setHealthOpen(false)}
              path={lead.path}
              faceIndex={lead.faceIndex}
              family={family.name}
              facesInFile={detail.facesInFile}
            />
          )}
        </motion.div>
      )}
    </AnimatePresence>
  );
}
