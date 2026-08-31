# TBG Pricing — self-updating desktop app (Tauri v2)

The browser viewer (`index.html`) is unchanged: double-click it and it still runs
offline. This folder now **also** packages it as a tiny Windows `.exe` that checks
GitHub for new versions, downloads them, and swaps itself for the new build when
you close it.

The app **ships empty** — no pricing data in the repo or the `.exe`. The boss
loads their own CSVs from the Upload screen at runtime. That's why the update
repo can stay **public with no token**: there is nothing sensitive to expose.

```
index.html            the app (browser + desktop share this one source)
webdist/              generated at build time (index.html + empty catalogue.js) — gitignored
src-tauri/            the desktop shell
  tauri.conf.json     window + bundle + updater config  ← has placeholders to fill
  src/lib.rs          update check / download / install-on-close logic
  capabilities/       window permissions
  icons/              app icon set
scripts/
  copy-web.mjs        index.html + empty catalogue.js → webdist/ (data-free)
  configure-updater.mjs  fills the repo + public key into tauri.conf.json
.github/workflows/release.yml   builds, signs and publishes a Release on every vX.Y.Z tag
```

## How updating works

1. On launch a background task asks
   `github.com/OWNER/REPO/releases/latest/download/latest.json` whether a newer,
   **signed** version exists.
2. If yes, the app downloads it silently and shows a bottom bar:
   *"Versión X lista — Reiniciar y actualizar … o se instalará al cerrar la app."*
3. **Reiniciar y actualizar** installs immediately and relaunches. Otherwise the
   download is applied automatically **when you close the window** — Windows can
   only replace a running `.exe` once it has exited, so the swap happens exactly
   "cuando el panel no está abierto".

No network / no release / a bad signature all just leave it running as the normal
offline viewer.

## One-time setup

### 0. Install the build toolchain (local builds only — CI already has it)
- **Rust**: <https://www.rust-lang.org/tools/install> (MSVC toolchain).
- Node is already installed. WebView2 ships with Windows 11.

### 1. Point the updater at your GitHub repo
```
node scripts/configure-updater.mjs <owner>/<repo>
```
This replaces `__GH_OWNER__` / `__GH_REPO__` in `tauri.conf.json`.

### 2. Generate the update signing keypair
```
npm install
npm run signer:generate          # writes tbg-pricing-updater.key (+ .key.pub)
```
- Copy the **public** key into the config:
  ```
  node scripts/configure-updater.mjs <owner>/<repo> tbg-pricing-updater.key.pub
  ```
- Add the **private** key + its password as GitHub repo secrets (Settings →
  Secrets and variables → Actions):
  - `TAURI_SIGNING_PRIVATE_KEY` = the full contents of `tbg-pricing-updater.key`
  - `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` = the password you chose
- **Never commit the `.key` file** (it's already in `.gitignore`).

### 3. First push
```
git init && git add . && git commit -m "TBG Pricing desktop app"
git branch -M main
git remote add origin https://github.com/<owner>/<repo>.git
git push -u origin main
```

## Cutting a release (this is what enables auto-update)
Bump the version in **both** `src-tauri/tauri.conf.json` and
`src-tauri/Cargo.toml`, then tag:
```
git commit -am "v0.2.0"
git tag v0.2.0
git push --tags
```
GitHub Actions builds the installer, signs it, and publishes a Release with
`latest.json`. Any running app on an older version will pick it up on next launch.

## Run / build locally
```
npm run dev      # hot-reload desktop window
npm run build    # produces src-tauri/target/release/bundle/nsis/*-setup.exe
```
> `npm run dev`/`build` auto-run `copy-web.mjs` first, so the desktop app always
> reflects the current `index.html` + `catalogue.js`.

## Notes
- The AI enrichment still runs from the webview with your saved key, exactly as
  in the browser — the desktop shell doesn't change that.
- Updates only flow from a **public** repo's releases (no token embedded in the
  distributed exe). If you ever make the repo private, updates need a proxy.
