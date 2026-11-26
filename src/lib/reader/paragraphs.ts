// Port of the Android app's NovelParagraphs.kt (splitNovelParagraphs) and
// NovelReaderViewModel.withChapterTitle. Reading progress is a paragraph index, so this must
// produce the same paragraph list as Android for the same chapter. Keep the two in sync.
//
// One JS gotcha: `\s` in JS also matches U+00A0 (nbsp); Java's `\s` doesn't, and the Kotlin
// rules depend on nbsp *not* being collapsed. So whitespace is spelled out as `W` here.

const W = "[ \\t\\n\\x0B\\f\\r]";

/** Block-level wrappers dropped when splitting; inline tags (`<b>`, `<i>`, ...) are kept. */
const STRIP_BLOCK_TAGS = /<\/?(p|h[1-6])[^>]*>/gi;
/** A tag that only ever appears in markup meant to be read as HTML, not as plain text. */
const STRUCTURAL_TAG = /<\/?(?:p|br|h[1-6]|div)\b/i;
const BR = "<br\\b[^>]*>";
const BR_REGEX = new RegExp(BR, "gi");
/** Paragraph boundaries in HTML mode: block ends, 2+ consecutive `<br>`, or a blank line. */
const HTML_BREAK_REGEX = new RegExp(`</p${W}*>|</h[1-6]${W}*>|${BR}(?:${W}*${BR})+|\\n{2,}`, "gi");
/** Plain-text paragraph boundaries. */
const TEXT_BREAK_REGEX = new RegExp(`</p>|</h[1-6]>|<br${W}*/?>${W}*<br${W}*/?>|\\n{2,}`, "gi");
const WHITESPACE_REGEX = new RegExp(`${W}+`, "g");
const BR_WITH_SPACE_REGEX = new RegExp(`${W}*${BR}${W}*`, "gi");
const EDGE_JUNK = `(?:${W}|\\u00A0|<br>|&nbsp;|&#160;|&#xa0;)+`;
const LEADING_JUNK_REGEX = new RegExp(`^${EDGE_JUNK}`, "i");
const TRAILING_JUNK_REGEX = new RegExp(`${EDGE_JUNK}$`, "i");
const ANY_TAG_REGEX = /<[^>]*>/g;
const NBSP_ENTITY_REGEX = /&nbsp;|&#160;|&#xa0;/gi;
const INLINE_TAG_REGEX = /<(\/?)(b|strong|i|em|u)\b[^>]*>/gi;

/** Lines in one block at which it is treated as a "one line per paragraph" chapter. */
const LINE_PER_PARAGRAPH_THRESHOLD = 3;

export function splitNovelParagraphs(text: string): string[] {
  if (!STRUCTURAL_TAG.test(text)) return splitPlainTextParagraphs(text);
  const out = splitHtmlParagraphs(text);
  return out.length ? out : [""];
}

function splitPlainTextParagraphs(text: string): string[] {
  const split = text
    .split(TEXT_BREAK_REGEX)
    .map((p) => p.replace(STRIP_BLOCK_TAGS, "").trim())
    .filter((p) => p.length > 0);
  return split.length ? split : [text.replace(STRIP_BLOCK_TAGS, "").trim()];
}

function splitHtmlParagraphs(text: string): string[] {
  const result: string[] = [];
  for (const chunk of text.split(HTML_BREAK_REGEX)) {
    const block = normalizeHtmlBlock(chunk.replace(STRIP_BLOCK_TAGS, ""));
    if (isBlankParagraph(block)) continue;
    const brCount = block.match(BR_REGEX)?.length ?? 0;
    if (brCount >= LINE_PER_PARAGRAPH_THRESHOLD) result.push(...splitAtLineBreaks(block));
    else result.push(block);
  }
  return result;
}

function normalizeHtmlBlock(block: string): string {
  return block
    .replace(WHITESPACE_REGEX, " ")
    .replace(BR_WITH_SPACE_REGEX, "<br>")
    .replace(LEADING_JUNK_REGEX, "")
    .replace(TRAILING_JUNK_REGEX, "");
}

function isBlankParagraph(s: string): boolean {
  return s.replace(ANY_TAG_REGEX, "").replace(NBSP_ENTITY_REGEX, "").replace(/\u00a0/g, " ").trim() === "";
}

function splitAtLineBreaks(block: string): string[] {
  const open: string[] = [];
  const result: string[] = [];
  for (const line of block.split(BR_REGEX)) {
    const prefix = open.map((t) => `<${t}>`).join("");
    for (const tag of line.matchAll(INLINE_TAG_REGEX)) {
      const name = tag[2].toLowerCase();
      if (tag[1] === "") open.push(name);
      else {
        const at = open.lastIndexOf(name);
        if (at >= 0) open.splice(at, 1);
      }
    }
    const suffix = [...open].reverse().map((t) => `</${t}>`).join("");
    const paragraph = prefix + line + suffix;
    if (!isBlankParagraph(paragraph)) result.push(paragraph.trim());
  }
  return result;
}

/** The chapter name is always paragraph 0; a matching first body paragraph is dropped. */
export function withChapterTitle(chapterName: string, body: string[]): string[] {
  const duplicate = body.length > 0 && isSameTitle(body[0], chapterName);
  return [chapterName, ...(duplicate ? body.slice(1) : body)];
}

function isSameTitle(paragraph: string, chapterName: string): boolean {
  const normalize = (v: string) =>
    v
      .replace(/<[^>]*>/g, "")
      .replace(/[^\p{L}\p{N}]/gu, "")
      .toLowerCase();
  const p = normalize(paragraph);
  return p.length > 0 && p === normalize(chapterName);
}

/** Same formula as Android's novelProgressPercent: truncated, 100 at the end. */
export function progressPercent(index: number, total: number, isAtEnd: boolean): number {
  if (total <= 1 || isAtEnd) return 100;
  return Math.min(100, Math.max(0, Math.trunc((index / (total - 1)) * 100)));
}
