# omatheme website

A static, dependency-free GitHub Pages site. HTML, CSS, and browser JavaScript; no build step, secrets, or runtime API requests.

## Preview

From the repository root:

```sh
python3 -m http.server 4173 --directory website
```

Open http://localhost:4173. You can also open `index.html` directly in a browser. CSS, JavaScript, and theme data are embedded, so no local server is needed. Wallpapers are embedded in the HTML and work offline. Optional web fonts use system fallbacks when offline.

## Publish on GitHub Pages

In the repository's **Settings → Pages → Build and deployment**, choose **GitHub Actions** as the source. Push this change to `main`, or run the **Website** workflow manually. The site will be at https://justinhhorner.github.io/omatheme/.

All internal paths are relative so the project subpath works. The Download links use `/releases/latest` and always lead to the current GitHub release.

## Editing

Edit `style.css`, `app.js`, or `themes.json`, then run `python3 website/build-page.py` to refresh their embedded copies in `index.html`. Commit the regenerated HTML alongside source changes.

## Theme collection

`themes.json` contains a curated snapshot of eight default themes from `omacom/omarchy`, including their source URLs and original named palettes. The wallpaper URL is the first image in each theme's `backgrounds/` directory, using natural filename order (the same convention as the apps). Rosé Pine is intentionally categorized as light: that is what its upstream `colors.toml` declares.

Optimized copies of the first wallpapers are stored in `assets/wallpapers/` and embedded into the generated HTML. Original image URLs are retained in `upstreamWallpaper`. Fonts load from Google Fonts with system font fallbacks; screenshots and the icon are local. The page reports wallpaper loading errors and keeps the last successful background. To refresh the collection, inspect the upstream directories, naturally sort their image names, and update the first wallpaper URL and palette together. Artwork belongs to its respective creators; the source repository is recorded with each theme. The included preview images are optimized copies of the upstream theme artwork, with provenance retained in the theme snapshot.

Theme selection is saved locally when storage is available. Page appearance and accent automatically follow the selected theme’s palette; there is no manual appearance toggle. Filters only change which themes are shown. Selecting a theme previews its first wallpaper in the page background; it does not change the visitor's desktop.

## Manual checks

- Select every theme and confirm its first wallpaper appears behind the hero.
- Rapidly select themes: only the most recent selection should update the background.
- Try All themes / Dark / Light, light and dark theme selections.
- Reload to check saved selection; also check with browser storage blocked.
- Check keyboard focus, mobile widths, reduced motion, and image/network failures.
- Confirm both Download links point to the latest release.
