import { describe, expect, it } from "vitest";
import { isSameRepo, repoCandidates } from "./repoLink";

const RAW = "https://raw.githubusercontent.com/someone/extensions";

describe("repoCandidates", () => {
  it("keeps an index URL as it is", () => {
    expect(repoCandidates(`${RAW}/repo/index.min.json`)).toEqual([`${RAW}/repo/index.min.json`]);
  });

  it("unwraps Add-to-Mihon style links", () => {
    const link = `mihon://add-repo?url=${encodeURIComponent(`${RAW}/repo/index.min.json`)}`;
    expect(repoCandidates(link)).toEqual([`${RAW}/repo/index.min.json`]);
    expect(repoCandidates(link.replace("mihon", "tachiyomi"))).toEqual([`${RAW}/repo/index.min.json`]);
  });

  it("turns a bare GitHub repo page into raw branch guesses", () => {
    expect(repoCandidates("https://github.com/someone/extensions")).toEqual([`${RAW}/repo`, `${RAW}/main`, `${RAW}/master`]);
  });

  it("maps GitHub tree, blob and raw pages to raw URLs", () => {
    expect(repoCandidates("https://github.com/someone/extensions/tree/repo")).toEqual([`${RAW}/repo`]);
    expect(repoCandidates("https://github.com/someone/extensions/blob/repo/index.min.json")).toEqual([`${RAW}/repo/index.min.json`]);
    expect(repoCandidates("https://github.com/someone/extensions/raw/repo/index.pb")).toEqual([`${RAW}/repo/index.pb`]);
  });

  it("adds https:// and trims quotes, spaces and trailing slashes", () => {
    expect(repoCandidates('  "example.org/mihon/"  ')).toEqual(["https://example.org/mihon"]);
    expect(repoCandidates("http://example.org/repo/index.json")).toEqual(["https://example.org/repo/index.json"]);
  });

  it("spots the same repo pasted in another form", () => {
    const have = ["https://github.com/someone/extensions/raw/repo/index.pb"];
    expect(isSameRepo("https://github.com/someone/extensions", have)).toBe(true);
    expect(isSameRepo(`${RAW}/repo/index.min.json`, have)).toBe(true);
    expect(isSameRepo("https://github.com/other/extensions", have)).toBe(false);
    expect(isSameRepo("not a link", have)).toBe(false);
  });

  it("rejects things that are not links", () => {
    expect(repoCandidates("")).toEqual([]);
    expect(repoCandidates("one piece")).toEqual([]);
    expect(repoCandidates("mihon://add-repo?foo=bar")).toEqual([]);
    expect(repoCandidates("file:///C:/x.json")).toEqual([]);
  });
});
