import { useSyncExternalStore } from "react";
// 界面只有简体中文。英文包留着当两件事：缺键兜底，以及 `Dict` 的类型基准
// （键集合和参数由它定义，中文包照着填），它不会出现在界面上。
import en from "../locales/en.json";
import zhCN from "../locales/zh-CN.json";

export type Dict = typeof en;
type Vars = Record<string, string | number>;

type PluralBase<K extends string> = K extends `${infer B}_one`
  ? B
  : K extends `${infer B}_other`
    ? B
    : never;

export type TKey = keyof Dict | PluralBase<keyof Dict & string>;

export type LocaleCode = "zh-CN";
export type LocalePref = LocaleCode | "system";

export interface LocaleInfo {
  code: LocaleCode;

  name: string;

  english: string;
}

export const LOCALES: readonly LocaleInfo[] = [
  { code: "zh-CN", name: "简体中文", english: "Chinese (Simplified)" },
];

const DICTS: Record<LocaleCode, Partial<Dict>> = {
  "zh-CN": zhCN,
};

const FALLBACK: LocaleCode = "zh-CN";
const STORAGE_KEY = "zfm.locale";

function isLocaleCode(v: unknown): v is LocaleCode {
  return typeof v === "string" && LOCALES.some((l) => l.code === v);
}

export function isLocalePref(v: unknown): v is LocalePref {
  return v === "system" || isLocaleCode(v);
}

export function detectLocale(): LocaleCode {
  return FALLBACK;
}

function readStoredPref(): LocalePref {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (isLocalePref(raw)) return raw;
  } catch {

  }
  return "system";
}

let pref: LocalePref = readStoredPref();
let resolved: LocaleCode = pref === "system" ? detectLocale() : pref;
let dict: Partial<Dict> = DICTS[resolved];
let plurals = new Intl.PluralRules(resolved);

let version = 0;
const listeners = new Set<() => void>();

function stampDocument() {
  if (typeof document !== "undefined") {
    document.documentElement.lang = resolved;
  }
}

function apply(next: LocalePref) {
  pref = next;
  resolved = next === "system" ? detectLocale() : next;
  dict = DICTS[resolved];
  plurals = new Intl.PluralRules(resolved);
  stampDocument();
  version++;
  for (const listen of listeners) listen();
}

stampDocument();

export function getLocalePref(): LocalePref {
  return pref;
}

export function activeLocale(): LocaleCode {
  return resolved;
}

export function setLocalePref(next: LocalePref) {
  if (next === pref) return;
  try {
    localStorage.setItem(STORAGE_KEY, next);
  } catch {

  }
  apply(next);
}

export function hydrateLocalePref(next: LocalePref) {
  try {
    localStorage.setItem(STORAGE_KEY, next);
  } catch {

  }
  if (next !== pref) apply(next);
}

function interpolate(template: string, vars: Vars): string {
  return template.replace(/\{(\w+)\}/g, (whole, name: string) =>
    name in vars ? String(vars[name]) : whole,
  );
}

function lookup(key: string): string | undefined {
  const local = (dict as Record<string, string | undefined>)[key];
  if (local !== undefined) return local;
  return (en as Record<string, string | undefined>)[key];
}

export function t(key: TKey, vars?: Vars): string {
  let raw: string | undefined;

  if (vars && typeof vars.count === "number") {
    const category = plurals.select(vars.count);
    raw =
      (dict as Record<string, string | undefined>)[`${key}_${category}`] ??
      (dict as Record<string, string | undefined>)[`${key}_other`] ??
      lookup(`${key}_${category}`) ??
      lookup(`${key}_other`);
  }

  raw ??= lookup(key);
  if (raw === undefined) return key;
  return vars ? interpolate(raw, vars) : raw;
}

function subscribe(listener: () => void) {
  listeners.add(listener);
  return () => {
    listeners.delete(listener);
  };
}

function getVersion() {
  return version;
}

export function useT(): typeof t {
  useSyncExternalStore(subscribe, getVersion, getVersion);
  return t;
}

export function useLocalePref(): LocalePref {
  useSyncExternalStore(subscribe, getVersion, getVersion);
  return pref;
}
