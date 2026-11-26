"""Builds assets/icon.svg from the Android app's adaptive launcher icon (vector drawables).

    python scripts/make_icon_svg.py D:/Coding/Kizuna
Then render to PNG and run `npm run tauri icon assets/icon.png`."""
import re, sys
from pathlib import Path
from xml.etree import ElementTree as ET

A = "{http://schemas.android.com/apk/res/android}"
res = Path(sys.argv[1]) / "app/src/main/res/drawable"
out = Path(__file__).resolve().parent.parent / "assets/icon.svg"

def paths(name):
    root = ET.parse(res / f"{name}.xml").getroot()
    for p in root.iter("path"):
        d = p.get(A + "pathData")
        fill = p.get(A + "fillColor", "#000000")
        if len(fill) == 9:  # #AARRGGBB -> #RRGGBB
            fill = "#" + fill[3:]
        rule = "evenodd" if p.get(A + "fillType") == "evenOdd" else "nonzero"
        yield f'<path d="{d}" fill="{fill}" fill-rule="{rule}"/>'

bg = list(paths("ic_launcher_background"))
fg = list(paths("ic_launcher_foreground"))
# Drop the full-square background fill; the desktop icon is a round badge like kizuna-logo.png.
bg = [p for p in bg if 'd="M0,0h432v432h-432z"' not in p]
svg = f'''<svg xmlns="http://www.w3.org/2000/svg" viewBox="66 63 300 300" width="1024" height="1024">
  <circle cx="216.07" cy="215.5" r="148" fill="#FAFAFA"/>
  {"".join(bg)}
  {"".join(fg)}
</svg>
'''
out.write_text(svg, encoding="utf-8")
print("wrote", out, "bg paths:", len(bg), "fg paths:", len(fg))
