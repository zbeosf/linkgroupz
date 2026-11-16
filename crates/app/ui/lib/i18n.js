// 화면 글의 언어 (한국어 · 영어). 앱이 정한 언어(boot.language)를 따른다 — 설정 → 언어, 기본은 시스템 언어.
//
// 글은 한국어로 적고 t("한국어") 로 감싼다. 영어는 i18n-en.js 의 표에서 한국어 글로 찾고, 없으면 한국어 그대로 보인다.
// {0} {1} … 자리에 인자를 넣는다: t("그룹 {0}개", count). 표의 값이 함수면 인자로 불러 쓴다 (단수 · 복수 등).

import { boot, listen } from "./api.js";
import { EN } from "./i18n-en.js";

export const lang = boot.language === "en" ? "en" : "ko";
document.documentElement.lang = lang;

export function t(text, ...args) {
  // 여러 줄에 걸친 글은 줄바꿈과 앞뒤 공백을 빈칸 하나로 보고 찾는다.
  const key = text.includes("\n") ? text.replace(/\s*\n\s*/g, " ") : text;
  const entry = lang === "en" ? EN[key] : undefined;
  if (typeof entry === "function") {
    return entry(...args);
  }
  const template = entry ?? text;
  return args.length === 0 ? template : template.replace(/\{(\d+)\}/g, (match, index) => String(args[Number(index)] ?? ""));
}

// 설정에서 언어를 바꾸면 모든 창을 다시 그린다.
listen("language-changed", () => location.reload());
