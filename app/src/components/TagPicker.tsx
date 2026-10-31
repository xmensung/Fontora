import { useState, type FormEvent } from "react";
import { Plus } from "lucide-react";
import { TAG_GROUPS, normalizeTag } from "../lib/tagTaxonomy";
import { useT } from "../lib/i18n";

/**
 * 分组词表 chips + 自由输入的标签点选面板。
 * 批量弹窗与字体详情页共用：selected 为归一化后的标签集合，
 * onToggle 负责把「显示值」交给外层去切换。
 */
export function TagPicker({
  selected,
  onToggle,
  autoFocusInput = false,
}: {
  selected: ReadonlySet<string>;
  onToggle: (tag: string) => void;
  autoFocusInput?: boolean;
}) {
  const t = useT();
  const [draft, setDraft] = useState("");

  const addDraft = (e?: FormEvent) => {
    e?.preventDefault();
    const clean = draft.trim();
    if (!clean) return;
    // 归一化后已选中的，回车视为取消选中；否则加入。
    onToggle(clean);
    setDraft("");
  };

  return (
    <div className="tagpicker">
      {TAG_GROUPS.map((group) => (
        <div className="tagpicker-group" key={group.id}>
          <div className="tagpicker-heading">{t(group.labelKey)}</div>
          <div className="tagpicker-chips">
            {group.tags.map((entry) => {
              const on = selected.has(normalizeTag(entry.value));
              return (
                <button
                  key={entry.value}
                  type="button"
                  className={`tagpicker-chip ${on ? "tagpicker-chip-on" : ""}`}
                  aria-pressed={on}
                  onClick={() => onToggle(entry.value)}
                >
                  {t(entry.labelKey)}
                </button>
              );
            })}
          </div>
        </div>
      ))}
      <form className="tagpicker-input-row" onSubmit={addDraft}>
        <input
          className="tagpicker-input"
          value={draft}
          onChange={(e) => setDraft(e.target.value)}
          placeholder={t("tag.addPlaceholder")}
          aria-label={t("tag.addAria")}
          spellCheck={false}
          autoFocus={autoFocusInput}
        />
        <button
          type="submit"
          className="tagpicker-add"
          disabled={!draft.trim()}
          aria-label={t("tag.addAria")}
        >
          <Plus size={13} strokeWidth={2} />
        </button>
      </form>
    </div>
  );
}
