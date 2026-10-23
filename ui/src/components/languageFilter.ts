import type { Language } from "../api";

/** Lower-case and strip diacritics so "viet" matches "Tiếng Việt". */
export function fold(s: string): string {
  return s.normalize("NFD").replace(/\p{M}/gu, "").replace(/đ/g, "d").replace(/Đ/g, "D").toLowerCase().trim();
}

/** Match by English name, native name (word prefix or substring) or exact-prefix code. */
export function matchesLanguage(lang: Language, query: string): boolean {
  const q = fold(query);
  if (!q) return true;
  return fold(lang.name).includes(q) || fold(lang.native).includes(q) || lang.code.toLowerCase().startsWith(q);
}
