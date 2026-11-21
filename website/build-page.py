#!/usr/bin/env python3
"""Embed the CSS, theme snapshot, and JS so index.html opens without a server."""
import json
import base64
import re
from pathlib import Path

root = Path(__file__).resolve().parent
page = root / 'index.html'
html = page.read_text()
icon = 'data:image/png;base64,' + base64.b64encode((root / 'assets/app-icon.png').read_bytes()).decode('ascii')
html = re.sub(r'<link rel="icon"[^>]*>', f'<link rel="icon" href="{icon}" type="image/png">', html)
html = re.sub(r'(<img id="header-app-icon" src=")[^"]*', lambda match: match[1] + icon, html)

def replace_section(html, name, content):
    start = f'<!-- INLINE_{name}_START -->'
    end = f'<!-- INLINE_{name}_END -->'
    before, remainder = html.split(start, 1)
    _, after = remainder.split(end, 1)
    return f'{before}{start}\n{content}\n  {end}{after}'

css = (root / 'style.css').read_text()
# Fonts are optional; loading them must never delay the page's own styling.
font_import, css = css.split('\n', 1)
font_url = font_import.split("'")[1]
html = replace_section(html, 'STYLE', f'  <style>\n{css}</style>\n  <link rel="stylesheet" href="{font_url.replace("&", "&amp;")}">')
collection = json.loads((root / 'themes.json').read_text())
for theme in collection:
    image = root / theme['wallpaper']
    theme['wallpaper'] = 'data:image/jpeg;base64,' + base64.b64encode(image.read_bytes()).decode('ascii')
data = json.dumps(collection, ensure_ascii=False).replace('<', '\\u003c')
js = (root / 'app.js').read_text()
html = replace_section(html, 'APP', f'  <script id="theme-data" type="application/json">{data}</script>\n  <script>\n{js}</script>')
page.write_text(html)
print('Built self-contained index.html styling and interactions.')
