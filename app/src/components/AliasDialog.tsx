import { AnimatePresence, motion } from "motion/react";
import { Tag, X } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { springSoft } from "../design/springs";
import { useFontStore } from "../state/fontStore";
import { useFocusTrap } from "../lib/useFocusTrap";
import { playTag } from "../lib/sound";
import { useT } from "../lib/i18n";

/**
 * 别名编辑：给一款字体加几个「你记得住的名字」。
 *
 * 别名进搜索（含拼音），所以给 `Source Han Sans SC` 加个「思源黑体」之后，
 * 打 `siyuan` 或 `syht` 都能找到它 —— 这是拼音检索里最实用的一块：
 * 系统里装的名字常常跟你想的不一样。
 */
export function AliasDialog() {
  const t = useT();
  const aliasFor = useFontStore((s) => s.aliasFor);
  const closeAlias = useFontStore((s) => s.closeAlias);
  const saveAliases = useFontStore((s) => s.saveAliases);
  const aliasesByName = useFontStore((s) => s.aliases);
  const trapRef = useFocusTrap<HTMLDivElement>(Boolean(aliasFor));
  const [draft, setDraft] = useState<string[]>([]);
  const [text, setText] = useState("");
  const [saving, setSaving] = useState(false);
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    if (!aliasFor) return;
    setDraft(aliasesByName[aliasFor] ?? []);
    setText("");
    const timer = setTimeout(() => inputRef.current?.focus(), 50);
    return () => clearTimeout(timer);
    // 只在打开时灌一次，不要在打字过程中被 store 变化覆盖。
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [aliasFor]);

  const add = () => {
    const value = text.trim();
    if (!value) return;
    if (value === aliasFor) {
      setText("");
      return;
    }
    if (draft.some((a) => a.toLowerCase() === value.toLowerCase()) || draft.length >= 8) {
      setText("");
      return;
    }
    playTag(true);
    setDraft([...draft, value]);
    setText("");
  };

  const commit = async () => {
    if (!aliasFor || saving) return;
    setSaving(true);
    await saveAliases(aliasFor, draft);
    setSaving(false);
    closeAlias();
  };

  return (
    <AnimatePresence>
      {aliasFor && (
        <motion.div
          className="compare-backdrop"
          initial={{ opacity: 0 }}
          animate={{ opacity: 1 }}
          exit={{ opacity: 0 }}
          transition={{ duration: 0.18 }}
          onClick={() => !saving && closeAlias()}
        >
          <motion.div
            ref={trapRef}
            className="alias-panel glass-e3"
            initial={{ y: 22, scale: 0.97, opacity: 0 }}
            animate={{ y: 0, scale: 1, opacity: 1 }}
            exit={{ y: 14, scale: 0.98, opacity: 0 }}
            transition={springSoft}
            onClick={(e) => e.stopPropagation()}
            role="dialog"
            aria-label={t("alias.title")}
          >
            <header className="alias-head">
              <h2 className="alias-title">
                <Tag size={14} strokeWidth={1.6} />
                {t("alias.title")}
              </h2>
              <button
                className="detail-close"
                aria-label={t("alias.cancel")}
                onClick={() => !saving && closeAlias()}
              >
                <X size={15} strokeWidth={1.5} />
              </button>
            </header>

            <p className="alias-sub">
              {t("alias.for")} <strong className="alias-family">{aliasFor}</strong>
            </p>

            {draft.length > 0 && (
              <div className="alias-chips">
                {draft.map((alias) => (
                  <button
                    key={alias}
                    className="alias-chip"
                    onClick={() => setDraft(draft.filter((a) => a !== alias))}
                    title={t("alias.remove")}
                  >
                    <span>{alias}</span>
                    <X size={10} strokeWidth={2.2} />
                  </button>
                ))}
              </div>
            )}

            <div className="topbar-field">
              <input
                ref={inputRef}
                value={text}
                maxLength={40}
                spellCheck={false}
                onChange={(e) => setText(e.target.value)}
                onKeyDown={(e) => {
                  if (e.key === "Enter") {
                    e.preventDefault();
                    if (text.trim()) add();
                    else void commit();
                  }
                  if (e.key === "Escape") {
                    e.preventDefault();
                    if (!saving) closeAlias();
                  }
                }}
                placeholder={t("alias.placeholder")}
                aria-label={t("alias.placeholder")}
              />
            </div>

            <p className="alias-hint">{t("alias.hint")}</p>

            <div className="alias-actions">
              <button className="alias-ghost" onClick={() => setDraft([])} disabled={!draft.length}>
                {t("alias.clearAll")}
              </button>
              <motion.button
                className="alias-save"
                onClick={() => void commit()}
                disabled={saving}
                whileHover={{ y: -1 }}
                whileTap={{ scale: 0.97 }}
              >
                {t("alias.save")}
              </motion.button>
            </div>
          </motion.div>
        </motion.div>
      )}
    </AnimatePresence>
  );
}
