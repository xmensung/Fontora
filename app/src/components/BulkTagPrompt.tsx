import { AnimatePresence, motion } from "motion/react";
import { Tag, X } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { springSoft } from "../design/springs";
import { useFontStore } from "../state/fontStore";
import { useFocusTrap } from "../lib/useFocusTrap";
import { playTag } from "../lib/sound";
import { useT } from "../lib/i18n";
import { TagPicker } from "./TagPicker";
import { normalizeTag } from "../lib/tagTaxonomy";

export function BulkTagPrompt() {
  const t = useT();
  const bulkTagFor = useFontStore((s) => s.bulkTagFor);
  const setBulkTagFor = useFontStore((s) => s.setBulkTagFor);
  const applyTagToFamilies = useFontStore((s) => s.applyTagToFamilies);
  const trapRef = useFocusTrap<HTMLDivElement>(Boolean(bulkTagFor));
  /** 本次要应用的标签集合（存显示值，应用时才归一化）。 */
  const [pending, setPending] = useState<string[]>([]);
  const panelRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (bulkTagFor) {
      setPending([]);
      setTimeout(() => panelRef.current?.querySelector("input")?.focus(), 50);
    }
  }, [bulkTagFor]);

  const toggle = (tag: string) => {
    const clean = tag.trim();
    if (!clean) return;
    const norm = normalizeTag(clean);
    setPending((cur) =>
      cur.some((x) => normalizeTag(x) === norm)
        ? cur.filter((x) => normalizeTag(x) !== norm)
        : [...cur, clean],
    );
  };

  const apply = () => {
    if (bulkTagFor && pending.length > 0) {
      playTag(true);
      for (const tag of pending) {
        void applyTagToFamilies(tag, bulkTagFor);
      }
    }
    setBulkTagFor(null);
  };

  const selectedNorms = new Set(pending.map(normalizeTag));

  return (
    <AnimatePresence>
      {bulkTagFor && (
        <motion.div
          className="compare-backdrop"
          initial={{ opacity: 0 }}
          animate={{ opacity: 1 }}
          exit={{ opacity: 0 }}
          transition={{ duration: 0.18 }}
          onClick={() => setBulkTagFor(null)}
        >
          <motion.div
            ref={trapRef}
            className="bulktag-panel glass-e3"
            initial={{ y: 24, scale: 0.97, opacity: 0 }}
            animate={{ y: 0, scale: 1, opacity: 1 }}
            exit={{ y: 16, scale: 0.98, opacity: 0 }}
            transition={springSoft}
            onClick={(e) => e.stopPropagation()}
            role="dialog"
            aria-label={t("bulk.aria")}
          >
            <header className="compare-head">
              <h2 className="bulktag-title">
                <Tag size={15} strokeWidth={1.5} />
                {t("bulk.title", { count: bulkTagFor.length })}
              </h2>
              <button
                className="detail-close"
                aria-label={t("bulk.cancel")}
                onClick={() => setBulkTagFor(null)}
              >
                <X size={15} strokeWidth={1.5} />
              </button>
            </header>

            {pending.length > 0 && (
              <div className="bulktag-selected">
                {pending.map((tag) => (
                  <button
                    key={tag}
                    className="bulktag-selected-chip"
                    onClick={() => toggle(tag)}
                    aria-label={t("tag.removeAria", { tag })}
                  >
                    {tag}
                    <X size={11} strokeWidth={2} />
                  </button>
                ))}
              </div>
            )}

            <div ref={panelRef} className="bulktag-picker">
              <TagPicker selected={selectedNorms} onToggle={toggle} />
            </div>

            <motion.button
              className="bulktag-apply"
              disabled={pending.length === 0}
              onClick={apply}
              whileHover={{ y: -1 }}
              whileTap={{ scale: 0.97 }}
            >
              {t("bulk.applyMany", { tags: pending.length, count: bulkTagFor.length })}
            </motion.button>
          </motion.div>
        </motion.div>
      )}
    </AnimatePresence>
  );
}
