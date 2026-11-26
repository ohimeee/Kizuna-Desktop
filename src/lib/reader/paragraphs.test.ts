// Ported from the Android app's NovelParagraphsTest.kt. The desktop splitter must produce the
// same paragraphs, because reading progress is a paragraph index.
import { describe, expect, it } from "vitest";
import { progressPercent, splitNovelParagraphs, withChapterTitle } from "./paragraphs";
import { paragraphHtml } from "./richtext";

const NBSP = " ";

function legacySplit(text: string): string[] {
  const strip = /<\/?(p|h[1-6])[^>]*>/gi;
  const split = text
    .split(/<\/p>|<\/h[1-6]>|<br[ \t\n\x0B\f\r]*\/?>[ \t\n\x0B\f\r]*<br[ \t\n\x0B\f\r]*\/?>|\n{2,}/gi)
    .map((s) => s.replace(strip, "").trim())
    .filter((s) => s.length > 0);
  return split.length ? split : [text.replace(strip, "").trim()];
}

const paragraphs = ["Chapter 1", "He opened the <i>door</i> slowly.", "“Who is there?” she asked.", "<b>Nobody</b> answered."];

describe("splitNovelParagraphs (parity with Android)", () => {
  it("compact one p per paragraph chapter splits as before", () => {
    const html = paragraphs.map((p) => `<p>${p}</p>`).join("");
    expect(splitNovelParagraphs(html)).toEqual(paragraphs);
    expect(splitNovelParagraphs(html)).toEqual(legacySplit(html));
  });

  it("jsoup pretty printed one p per paragraph chapter splits as before", () => {
    const html = paragraphs.map((p) => `  <p>${p}</p>`).join("\n") + "\n";
    expect(splitNovelParagraphs(html)).toEqual(paragraphs);
    expect(splitNovelParagraphs(html)).toEqual(legacySplit(html));
  });

  it("a wrapping div keeps the same paragraph count and order", () => {
    const html = '<div class="chapter">\n' + paragraphs.map((p) => `  <p>${p}</p>`).join("\n") + "\n</div>";
    const now = splitNovelParagraphs(html);
    const old = legacySplit(html);
    expect(old[old.length - 1]).toBe("</div>");
    expect(now.length).toBe(old.length - 1);
    expect(now.slice(1)).toEqual(old.slice(1, -1));
    expect(now[0]).toBe('<div class="chapter"> Chapter 1');
  });

  it("headings still end a paragraph", () => {
    const html = "<h4>Chapter 2</h4><p>First sentence.</p><p>Second.</p>";
    expect(splitNovelParagraphs(html)).toEqual(["Chapter 2", "First sentence.", "Second."]);
    expect(splitNovelParagraphs(html)).toEqual(legacySplit(html));
  });

  it("double br is still a paragraph break", () => {
    const html = "<p>One.<br><br>Two.<br>\n  <br>\n  Three.</p>";
    expect(splitNovelParagraphs(html)).toEqual(["One.", "Two.", "Three."]);
    expect(splitNovelParagraphs(html)).toEqual(legacySplit(html));
  });

  it("pretty printed one p with many br gives one clean paragraph per line", () => {
    const html = "<p>Chapter 1\n  <br>\n  “What a mess.”\n  <br>\n  He sighed.\n  <br>\n  <i>Not again.</i></p>";
    expect(splitNovelParagraphs(html)).toEqual(["Chapter 1", "“What a mess.”", "He sighed.", "<i>Not again.</i>"]);
  });

  it("an inline tag spanning br lines is closed and reopened per line", () => {
    expect(splitNovelParagraphs("<p>a<br><i>one<br>two<br>three</i><br>four</p>")).toEqual([
      "a", "<i>one</i>", "<i>two</i>", "<i>three</i>", "four",
    ]);
  });

  it("one or two br stay as inline line breaks", () => {
    expect(splitNovelParagraphs("<p>Q. Why?\n  <br>\n  A. No.</p>")).toEqual(["Q. Why?<br>A. No."]);
    expect(splitNovelParagraphs("<p>a<br>b<br>c</p>")).toEqual(["a<br>b<br>c"]);
  });

  it("whitespace runs collapse and source newlines are not line breaks", () => {
    expect(splitNovelParagraphs("<p>  one \n\t two   three  </p>")).toEqual(["one two three"]);
    expect(splitNovelParagraphs("<p>one\n  two</p>")).toEqual(["one two"]);
  });

  it("non breaking spaces inside text survive and blank paragraphs are dropped", () => {
    expect(splitNovelParagraphs(`<p>a${NBSP}${NBSP}b</p>`)).toEqual([`a${NBSP}${NBSP}b`]);
    expect(splitNovelParagraphs("<p>a&nbsp;&nbsp;b</p>")).toEqual(["a&nbsp;&nbsp;b"]);
    expect(splitNovelParagraphs(`<p>One</p><p>&nbsp;</p><p>${NBSP} </p><p><br></p><p>Two</p>`)).toEqual(["One", "Two"]);
  });

  it("empty html yields a single blank paragraph for the caller's empty check", () => {
    expect(splitNovelParagraphs("<p></p>").every((p) => p.trim() === "")).toBe(true);
  });

  it("plain text is split on blank lines only and keeps single newlines", () => {
    const text = "Chapter 1\nline two\n\nSecond paragraph, with <i>italics</i> and a < b.\n\n\nThird.";
    expect(splitNovelParagraphs(text)).toEqual([
      "Chapter 1\nline two",
      "Second paragraph, with <i>italics</i> and a < b.",
      "Third.",
    ]);
    expect(splitNovelParagraphs(text)).toEqual(legacySplit(text));
  });

  it("plain text with many newlines is not mistaken for line per paragraph html", () => {
    const text = "a\nb\nc\nd\ne";
    expect(splitNovelParagraphs(text)).toEqual([text]);
    expect(splitNovelParagraphs(text)).toEqual(legacySplit(text));
  });
});

describe("withChapterTitle", () => {
  it("prepends the chapter name and drops a duplicate heading", () => {
    expect(withChapterTitle("Chapter 1: Start", ["<b>Chapter 1 - Start</b>", "Body"])).toEqual(["Chapter 1: Start", "Body"]);
    expect(withChapterTitle("Chapter 1", ["Body"])).toEqual(["Chapter 1", "Body"]);
  });
});

describe("progressPercent", () => {
  it("truncates and reports 100 at the end", () => {
    expect(progressPercent(1, 3, false)).toBe(50);
    expect(progressPercent(2, 4, false)).toBe(66);
    expect(progressPercent(0, 1, false)).toBe(100);
    expect(progressPercent(3, 10, true)).toBe(100);
  });
});

describe("paragraphHtml", () => {
  it("keeps b/i/u, drops other tags, escapes text, decodes entities once", () => {
    expect(paragraphHtml('<strong>Hi</strong> <span class="x">there</span> <em>you</em>')).toBe("<b>Hi</b> there <i>you</i>");
    expect(paragraphHtml("a &lt;script&gt; &amp;lt; &ldquo;q&rdquo;")).toBe("a &lt;script&gt; &amp;lt; “q”");
    expect(paragraphHtml("<script>alert(1)</script>")).toBe("alert(1)");
    expect(paragraphHtml("<b>unclosed")).toBe("<b>unclosed</b>");
    expect(paragraphHtml("stray</i> close")).toBe("stray close");
  });

  it("bionic bolds about half of each word", () => {
    expect(paragraphHtml("reading is fun", true)).toBe('<b class="bionic">rea</b>ding <b class="bionic">i</b>s <b class="bionic">f</b>un');
  });
});
