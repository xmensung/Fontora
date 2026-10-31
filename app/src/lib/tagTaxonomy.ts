import type { TKey } from "./i18n";

/**
 * 标签分组词表。标签本身只存简短的纯字符串（如 "宋体"），分组关系只存在于
 * 前端常量里 —— 存储层零迁移，用户已有的自定义标签自动落入「其他」。
 */

export interface TagEntry {
  /** 实际写入存储层的标签值（规范化前的显示值）。 */
  value: string;
  labelKey: TKey;
}

export interface TagGroup {
  id: string;
  labelKey: TKey;
  tags: TagEntry[];
}

export const TAG_GROUPS: TagGroup[] = [
  {
    id: "origin",
    labelKey: "taggroup.origin",
    tags: [
      { value: "宋体", labelKey: "tagval.songti" },
      { value: "仿宋", labelKey: "tagval.fangsong" },
      { value: "黑体", labelKey: "tagval.heiti" },
      { value: "楷体", labelKey: "tagval.kaiti" },
      { value: "圆体", labelKey: "tagval.yuanti" },
      { value: "隶书", labelKey: "tagval.lishu" },
      { value: "篆书", labelKey: "tagval.zhuanshu" },
      { value: "行书", labelKey: "tagval.xingshu" },
      { value: "草书", labelKey: "tagval.caoshu" },
      { value: "西文衬线", labelKey: "tagval.serif" },
      { value: "西文无衬线", labelKey: "tagval.sans" },
      { value: "手写体", labelKey: "tagval.script" },
      { value: "装饰体", labelKey: "tagval.display" },
    ],
  },
  {
    id: "mood",
    labelKey: "taggroup.mood",
    tags: [
      { value: "正文", labelKey: "tagval.body" },
      { value: "标题", labelKey: "tagval.heading" },
      { value: "书法手写", labelKey: "tagval.calligraphy" },
      { value: "艺术展示", labelKey: "tagval.artistic" },
      { value: "品牌海报", labelKey: "tagval.brand" },
      { value: "屏幕UI", labelKey: "tagval.screenui" },
      { value: "印刷出版", labelKey: "tagval.print" },
      { value: "复古怀旧", labelKey: "tagval.retro" },
      { value: "商务严肃", labelKey: "tagval.business" },
      { value: "活泼儿童", labelKey: "tagval.kids" },
    ],
  },
  {
    id: "attrs",
    labelKey: "taggroup.attrs",
    tags: [
      { value: "等宽", labelKey: "tagval.monospaced" },
      { value: "斜体", labelKey: "tagval.italic" },
      { value: "中英混排", labelKey: "tagval.mixed" },
      { value: "可变字体", labelKey: "tagval.variable" },
      { value: "繁体支持", labelKey: "tagval.traditional" },
      { value: "生僻字支持", labelKey: "tagval.rareHan" },
      { value: "大字库", labelKey: "tagval.bigCjk" },
      { value: "专业排版", labelKey: "tagval.opentype" },
    ],
  },
];

/** 与 fontStore.applyTagToFamilies 相同的归一化规则。 */
export function normalizeTag(tag: string): string {
  return tag.trim().toLowerCase();
}

/** 归一化后的标签值 → 所属分组 id；未命中词表返回 null（归入「其他」）。 */
export function groupIdOfTag(tag: string): string | null {
  const n = normalizeTag(tag);
  for (const group of TAG_GROUPS) {
    if (group.tags.some((t) => normalizeTag(t.value) === n)) return group.id;
  }
  return null;
}
