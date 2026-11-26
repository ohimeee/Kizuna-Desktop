// Port of the Android app's NovelRichText.kt: a paragraph's inline HTML becomes safe HTML that
// keeps only bold/italic/underline (+ <br>). Every other tag is dropped, entities are decoded
// in a single pass, and the output is escaped, so source markup can never inject anything.

const ANY_TAG_REGEX = /<(\/?)\s*([a-zA-Z0-9]+)[^>]*>/g;
const ENTITY_REGEX = /&(#[xX][0-9a-fA-F]+|#[0-9]+|[a-zA-Z][a-zA-Z0-9]*);/g;

const NAMED_ENTITIES: Record<string, string> = {
  amp: "&", lt: "<", gt: ">", quot: '"', apos: "'", nbsp: "\u00a0", mdash: "—", ndash: "–",
  hellip: "…", ldquo: "“", rdquo: "”", lsquo: "‘", rsquo: "’", laquo: "«", raquo: "»",
  bull: "•", middot: "·", deg: "°", times: "×", divide: "÷", copy: "©", reg: "®", trade: "™",
  prime: "′", Prime: "″", dagger: "†", Dagger: "‡", permil: "‰", euro: "€", pound: "£",
  yen: "¥", cent: "¢", sect: "§", para: "¶", ensp: "\u2002", emsp: "\u2003", thinsp: "\u2009",
  zwj: "\u200d", zwnj: "\u200c", shy: "\u00ad",
};

function codePoint(n: number): string | null {
  return Number.isFinite(n) && n >= 1 && n <= 0x10ffff ? String.fromCodePoint(n) : null;
}

export function decodeEntities(text: string): string {
  if (!text.includes("&")) return text;
  return text.replace(ENTITY_REGEX, (m, body: string) => {
    if (body[0] === "#") {
      const n = body[1] === "x" || body[1] === "X" ? parseInt(body.slice(2), 16) : parseInt(body.slice(1), 10);
      return codePoint(n) ?? m;
    }
    return NAMED_ENTITIES[body] ?? m;
  });
}

function escapeHtml(s: string): string {
  return s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
}

type Token = { kind: "text"; text: string } | { kind: "open"; tag: "b" | "i" | "u" } | { kind: "close" } | { kind: "br" };

function tokenize(html: string): Token[] {
  const tokens: Token[] = [];
  let cursor = 0;
  for (const m of html.matchAll(ANY_TAG_REGEX)) {
    const start = m.index ?? 0;
    if (start > cursor) tokens.push({ kind: "text", text: decodeEntities(html.slice(cursor, start)) });
    const closing = m[1] === "/";
    const tag = m[2].toLowerCase();
    const styled = tag === "b" || tag === "strong" || tag === "i" || tag === "em" || tag === "u";
    if (tag === "br") tokens.push({ kind: "br" });
    else if (closing && styled) tokens.push({ kind: "close" });
    else if (tag === "b" || tag === "strong") tokens.push({ kind: "open", tag: "b" });
    else if (tag === "i" || tag === "em") tokens.push({ kind: "open", tag: "i" });
    else if (tag === "u") tokens.push({ kind: "open", tag: "u" });
    cursor = start + m[0].length;
  }
  if (cursor < html.length) tokens.push({ kind: "text", text: decodeEntities(html.slice(cursor)) });
  return tokens;
}

/**
 * Safe HTML for one paragraph. With `bionic`, roughly the first half of each word is bolded
 * (words of 1-3 letters get one bold letter), on top of the source's own formatting.
 */
export function paragraphHtml(html: string, bionic = false): string {
  const tokens = tokenize(html);

  // Bionic: which character offsets (in the rendered text) are bold.
  let boldAt: ((offset: number) => boolean) | null = null;
  if (bionic) {
    const plain = tokens.map((t) => (t.kind === "text" ? t.text : t.kind === "br" ? "\n" : "")).join("");
    const bold = new Uint8Array(plain.length);
    for (const w of plain.matchAll(/\S+/g)) {
      const len = w[0].length;
      const n = len <= 3 ? 1 : Math.max(1, Math.trunc(len * 0.5));
      bold.fill(1, w.index ?? 0, (w.index ?? 0) + n);
    }
    boldAt = (o) => bold[o] === 1;
  }

  let out = "";
  const open: string[] = [];
  let offset = 0;
  for (const t of tokens) {
    switch (t.kind) {
      case "text": {
        if (!boldAt) {
          out += escapeHtml(t.text);
        } else {
          // Group runs of same boldness.
          let run = "";
          let runBold = false;
          const flush = () => {
            if (run) out += runBold ? `<b class="bionic">${escapeHtml(run)}</b>` : escapeHtml(run);
            run = "";
          };
          for (let i = 0; i < t.text.length; i++) {
            const b = boldAt(offset + i);
            if (b !== runBold) { flush(); runBold = b; }
            run += t.text[i];
          }
          flush();
        }
        offset += t.text.length;
        break;
      }
      case "br":
        out += "<br>";
        offset += 1;
        break;
      case "open":
        out += `<${t.tag}>`;
        open.push(t.tag);
        break;
      case "close": {
        // Like Android's AnnotatedString pop(): closes the most recent open style; stray close tags are ignored.
        const tag = open.pop();
        if (tag) out += `</${tag}>`;
        break;
      }
    }
  }
  while (open.length) out += `</${open.pop()}>`;
  return out;
}

/** Plain text of a paragraph (for title-length checks etc). */
export function paragraphText(html: string): string {
  return tokenize(html).map((t) => (t.kind === "text" ? t.text : t.kind === "br" ? "\n" : "")).join("");
}
