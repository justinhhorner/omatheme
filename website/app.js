'use strict';
const root = document.documentElement;
const grid = document.querySelector('#theme-grid');
const status = document.querySelector('#gallery-status');
const wallpaper = document.querySelector('#wallpaper');
let themes = [];
let selected;
let filter = 'all';
let selectionVersion = 0;
const readPreference = key => { try { return localStorage.getItem(key); } catch { return null; } };
const savePreference = (key, value) => { try { localStorage.setItem(key, value); } catch { /* Storage is optional. */ } };
function setAppearance(theme) {
  const appearance = theme.colors.mode === 'light' ? 'light' : 'dark';
  root.dataset.appearance = appearance;
  root.style.setProperty('--accent', theme.colors.accent);
  document.querySelector('meta[name="theme-color"]').content = appearance === 'light' ? '#f5f7fa' : '#101216';
}
function render() {
  grid.replaceChildren();
  for (const theme of themes.filter(t => filter === 'all' || t.colors.mode === filter)) {
    const card = document.createElement('button');
    card.type = 'button';
    card.className = 'theme-card';
    card.dataset.slug = theme.slug;
    card.style.setProperty('--theme-bg', theme.colors.background);
    card.setAttribute('aria-pressed', String(theme.slug === selected?.slug));
    card.setAttribute('aria-label', `Preview ${theme.name}, ${theme.colors.mode} theme`);
    const image = document.createElement('img');
    image.className = 'theme-image'; image.src = theme.wallpaper; image.alt = `${theme.name} first wallpaper`; image.loading = 'lazy'; image.decoding = 'async';
    const details = document.createElement('div'); details.className = 'theme-details';
    const title = document.createElement('div'); title.className = 'theme-title';
    const name = document.createElement('span'); name.textContent = theme.name;
    const mode = document.createElement('span'); mode.className = 'theme-mode'; mode.textContent = theme.colors.mode;
    title.append(name, mode);
    const swatches = document.createElement('div'); swatches.className = 'swatches'; swatches.setAttribute('aria-hidden', 'true');
    for (const key of ['background', 'red', 'yellow', 'green', 'blue', 'magenta', 'cyan']) {
      const swatch = document.createElement('span'); swatch.style.setProperty('--swatch', theme.colors[key]); swatches.append(swatch);
    }
    const mark = document.createElement('span'); mark.className = 'selection-mark'; mark.textContent = '✓'; mark.setAttribute('aria-hidden', 'true');
    details.append(title, swatches); card.append(image, details, mark);
    card.addEventListener('click', () => {
      selectTheme(theme);
      window.scrollTo({ top: 0, left: 0, behavior: 'auto' });
    });
    grid.append(card);
  }
}
function selectTheme(theme) {
  selected = theme;
  savePreference('omatheme-theme', theme.slug);
  setAppearance(theme);
  for (const card of grid.children) card.setAttribute('aria-pressed', String(card.dataset.slug === theme.slug));
  const version = ++selectionVersion;
  status.textContent = `Loading ${theme.name}’s first wallpaper…`;
  const image = new Image();
  image.onload = () => {
    if (version !== selectionVersion) return;
    wallpaper.style.backgroundImage = `url("${theme.wallpaper}")`;
    document.querySelector('#display-theme').textContent = theme.name;
    status.textContent = `${theme.name} selected.`;
  };
  image.onerror = () => { if (version === selectionVersion) status.textContent = `Couldn’t load ${theme.name}’s wallpaper. Check your connection or try another theme.`; };
  image.src = theme.wallpaper;
}
for (const button of document.querySelectorAll('[data-filter]')) button.addEventListener('click', () => {
  filter = button.dataset.filter;
  for (const item of document.querySelectorAll('[data-filter]')) item.setAttribute('aria-pressed', String(item === button));
  render();
});
Promise.resolve().then(() => JSON.parse(document.querySelector('#theme-data').textContent)).then(collection => {
  themes = collection;
  selected = themes.find(t => t.slug === readPreference('omatheme-theme')) || themes[0];
  render(); selectTheme(selected);
}).catch(() => { status.textContent = 'The collection couldn’t load. Refresh to try again, or explore the community collection below.'; });
