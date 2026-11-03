import type { TKey } from "./i18n";

/**
 * 预览文本预设。
 *
 * 挑文本的原则不是「好看」，是「能暴露问题」：
 * 标点组看引号书名号是否等宽、偏旁组看部首是否被偷懒做成简笔、
 * 生僻字组直接翻 CJK 扩展 B 区、拉丁组看数字有没有做旧式对齐。
 */
export interface SamplePreset {
  id: string;
  labelKey: TKey;
  text: string;
}

export const SAMPLE_PRESETS: readonly SamplePreset[] = [
  {
    id: "modern",
    labelKey: "preview.preset.modern",
    text: "春风又绿江南岸，明月何时照我还。",
  },
  {
    id: "traditional",
    labelKey: "preview.preset.traditional",
    text: "春風又綠江南岸，明月何時照我還。",
  },
  {
    id: "punct",
    labelKey: "preview.preset.punct",
    text: "，。、；：？！「」『』（）《》〈〉——……·～﹏　",
  },
  {
    id: "radical",
    labelKey: "preview.preset.radical",
    text: "亻彳氵冫扌讠忄阝刂勹廾弋彐宀广疒衤礻罒耂虍覀贝镸門隹雨革韋頁食馬骨高髟鬼魚鳥鹿麥黃黑鼓鼠鼻齊齒龍龜",
  },
  {
    id: "latin",
    labelKey: "preview.preset.latin",
    text: "Hamburgefonstiv ABCDEFG abcdefg",
  },
  {
    id: "kana",
    labelKey: "preview.preset.kana",
    text: "いろはにほへと ちりぬるを アイウエオ カキクケコ",
  },
  {
    id: "rare",
    labelKey: "preview.preset.rare",
    text: "𠀀𠀁𠀂𠀃𠮷𪚥𫝀𬺰𠈌𪜀",
  },
  {
    id: "symbols",
    labelKey: "preview.preset.symbols",
    text: "0123456789 $ € £ ¥ ½ ¼ ¾ № © ® ™ ° ‰",
  },
];

export function presetById(id: string | null): SamplePreset | undefined {
  if (!id) return undefined;
  return SAMPLE_PRESETS.find((p) => p.id === id);
}
