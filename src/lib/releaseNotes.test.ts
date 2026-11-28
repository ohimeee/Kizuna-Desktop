import { describe, expect, it } from "vitest";
import { renderReleaseNotes } from "./releaseNotes";

describe("renderReleaseNotes", () => {
  it("renders headings, bullets, bold and code", () => {
    const html = renderReleaseNotes("Intro line.\n\n### New\n- **Badges:** see `More`\n- Second");
    expect(html).toBe("<p>Intro line.</p><h3>New</h3><ul><li><strong>Badges:</strong> see <code>More</code></li><li>Second</li></ul>");
  });

  it("drops the installer help paragraph", () => {
    const html = renderReleaseNotes("Summary.\n\n**Install:** download `x.exe` and run it.\n\n### New\n- A");
    expect(html).not.toContain("Install");
    expect(html).toContain("<p>Summary.</p>");
  });

  it("escapes HTML so notes cannot inject markup", () => {
    const html = renderReleaseNotes('- <img src=x onerror="alert(1)"> & more');
    expect(html).toBe("<ul><li>&lt;img src=x onerror=&quot;alert(1)&quot;&gt; &amp; more</li></ul>");
  });

  it("shows links as their text", () => {
    expect(renderReleaseNotes("See [the guide](https://example.com).")).toBe("<p>See the guide.</p>");
  });

  it("joins wrapped bullet lines and handles CRLF", () => {
    expect(renderReleaseNotes("- one\r\n  continued\r\n- two")).toBe("<ul><li>one continued</li><li>two</li></ul>");
  });
});
