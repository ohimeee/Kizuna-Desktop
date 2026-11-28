// Smart paste for manga extension repos: turns whatever a newcomer copied into the URLs worth
// trying, best guess first. The Rust side (manga::fetch_repo) then reads index.json /
// index.min.json under each one, so a candidate may be a repo folder or an index file.
//
// Accepted: an index URL (…/index.min.json, …/index.json, …/repo.json, …/index.pb); a repo
// folder; an "Add to Mihon" style link (mihon://add-repo?url=…, also tachiyomi/aniyomi/komikku);
// a GitHub page (github.com/owner/repo, …/tree/branch/path, …/blob/branch/file, …/raw/…); and
// any of those without the https:// in front.

const DEEP_LINK = /^(?:mihon|tachiyomi|aniyomi|komikku|kizuna):\/\/add-repo\/?\?(.*)$/i;

export function repoCandidates(input: string): string[] {
  let s = input.trim().replace(/^["'<]+|["'>]+$/g, "");
  if (!s) return [];

  const deep = s.match(DEEP_LINK);
  if (deep) {
    const url = new URLSearchParams(deep[1]).get("url");
    if (!url) return [];
    s = url.trim();
  }
  if (!/^[a-z][a-z0-9+.-]*:\/\//i.test(s)) {
    if (!/^[\w-]+(\.[\w-]+)+(\/|$)/.test(s)) return []; // not even host-shaped
    s = "https://" + s;
  }
  let u: URL;
  try { u = new URL(s); } catch { return []; }
  if (u.protocol === "http:") u.protocol = "https:";
  if (u.protocol !== "https:") return [];

  const host = u.hostname.replace(/^www\./, "");
  const parts = u.pathname.split("/").filter(Boolean);
  const out: string[] = [];
  const add = (x: string) => { const v = x.replace(/\/+$/, ""); if (!out.includes(v)) out.push(v); };

  if (host === "github.com" && parts.length >= 2) {
    const [owner, repo, kind, branch, ...rest] = parts;
    const raw = `https://raw.githubusercontent.com/${owner}/${repo.replace(/\.git$/, "")}`;
    if ((kind === "blob" || kind === "tree" || kind === "raw") && branch) {
      add(`${raw}/${branch}${rest.length ? "/" + rest.join("/") : ""}`);
    } else {
      // a bare repo page: the index usually sits on a "repo" branch (the common layout for
      // extension repos), otherwise at the root of the default branch
      for (const b of ["repo", "main", "master"]) add(`${raw}/${b}`);
    }
    return out;
  }
  add(u.origin + u.pathname + (u.search || ""));
  return out;
}

// The repo folder a link points at, for spotting the same repo pasted in another form
// (its GitHub page, its index.pb, its index.min.json…).
const folder = (url: string) => url.replace(/\/(index(\.min)?\.json|index\.pb|repo\.json)$/i, "").toLowerCase();

/** True when `input` names a repo that is already in `repos`, in any accepted form. */
export function isSameRepo(input: string, repos: string[]): boolean {
  const have = new Set(repos.flatMap((r) => repoCandidates(r).map(folder)));
  const cands = repoCandidates(input).map(folder);
  // a bare GitHub page yields several branch guesses; only the first is the usual layout
  return cands.length > 0 && (have.has(cands[0]) || cands.every((c) => have.has(c)));
}
