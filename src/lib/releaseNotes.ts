// Turns a GitHub release's Markdown notes into a small, safe HTML fragment for the update dialog.
// Only what release notes use: "##"/"###" headings, "-"/"*" bullets, paragraphs, **bold**,
// `code` and [text](url) links (shown as their text: the dialog never navigates). Everything is
// HTML-escaped first, so nothing in the notes can inject markup.

const esc = (s: string) => s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;").replace(/"/g, "&quot;");

const inline = (s: string) =>
  esc(s)
    .replace(/`([^`]+)`/g, "<code>$1</code>")
    .replace(/\*\*([^*]+)\*\*/g, "<strong>$1</strong>")
    .replace(/\[([^\]]+)\]\([^)]*\)/g, "$1");

// The "**Install:** download the setup.exe…" paragraph is for people on the release page; inside
// the app the Update button does that, so it is left out.
const isInstallHelp = (p: string) => /^\*\*install[^*]*\*\*/i.test(p.trim());

export function renderReleaseNotes(md: string): string {
  const out: string[] = [];
  let list: string[] = [];
  let para: string[] = [];
  const flushList = () => { if (list.length) out.push(`<ul>${list.map((li) => `<li>${li}</li>`).join("")}</ul>`); list = []; };
  const flushPara = () => {
    const text = para.join(" ").trim();
    if (text && !isInstallHelp(text)) out.push(`<p>${inline(text)}</p>`);
    para = [];
  };
  for (const raw of md.replace(/\r\n?/g, "\n").split("\n")) {
    const line = raw.trimEnd();
    const h = line.match(/^#{1,4}\s+(.*)$/);
    const li = line.match(/^\s*[-*]\s+(.*)$/);
    if (h) { flushPara(); flushList(); out.push(`<h3>${inline(h[1])}</h3>`); }
    else if (li) { flushPara(); list.push(inline(li[1])); }
    else if (!line.trim()) { flushPara(); flushList(); }
    else if (list.length && /^\s{2,}\S/.test(raw)) list[list.length - 1] += " " + inline(line.trim()); // a wrapped bullet
    else { flushList(); para.push(line); }
  }
  flushPara();
  flushList();
  return out.join("");
}
