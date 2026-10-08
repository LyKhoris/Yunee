# Yunee — Flatpak

Yunee ships as a Flatpak. This directory holds the manifest; the app, its
desktop entry, icon, and AppStream metadata live under `data/`.

## Requirements

```bash
sudo dnf install flatpak flatpak-builder          # Fedora
flatpak remote-add --user --if-not-exists flathub \
    https://flathub.org/repo/flathub.flatpakrepo
```

The build uses `org.gnome.Platform//51` (GTK 4.24 / libadwaita 1.10) and the
`org.freedesktop.Sdk.Extension.rust-stable` SDK extension; `flatpak-builder
--install-deps-from=flathub` fetches them on first run.

The build sandbox has **no network**, so crates must be vendored first:

```bash
cargo vendor vendor          # writes vendor/ (gitignored)
```

## Build and install (local)

```bash
cargo vendor vendor
flatpak-builder --user --install --force-clean \
    --install-deps-from=flathub \
    build-dir flatpak/io.github.LyKhoris.Yunee.yml

flatpak run io.github.LyKhoris.Yunee
```

## Build a single-file bundle (for a release)

```bash
cargo vendor vendor
flatpak-builder --user --force-clean \
    --install-deps-from=flathub \
    --repo=repo build-dir flatpak/io.github.LyKhoris.Yunee.yml

flatpak build-bundle repo yunee-0.1.0-beta.1.flatpak io.github.LyKhoris.Yunee
```

The resulting `.flatpak` installs on any machine with:

```bash
flatpak install yunee-0.1.0-beta.1.flatpak
```

Releases are cut by pushing a tag (`v*`); `.github/workflows/release.yml`
builds the bundle and attaches it to the GitHub release.

## Updates

There is **no Flatpak repository and no update check**. Each release carries the
`.flatpak` bundle as an asset, and **Settings → Updates** has a *Latest release*
button that opens the releases page on GitHub in the browser. Download the newer
bundle there and install it:

```bash
flatpak install --user ./yunee-<version>.flatpak
```

(The app does not call the GitHub API: the repository is private, so a check
would need credentials the app does not ask for.)

## Permissions

| Permission | Why |
|---|---|
| `--share=network` | Syncing with Canvas over HTTPS |
| `--socket=wayland`, `--socket=fallback-x11`, `--share=ipc`, `--device=dri` | Display |
| `--talk-name=org.freedesktop.notifications` | New-announcement notifications |
| `--talk-name=org.freedesktop.secrets` | The GNOME keyring, where the Canvas token is stored |
| `--filesystem=xdg-download` | Saving files downloaded from Canvas |

## Offline builds

The manifest builds offline from the vendored crates in `vendor/` (written by
`cargo vendor`, gitignored, regenerated in CI). That is what the Flatpak build
sandbox requires — it has no network.

For **Flathub**, the convention is instead to vendor into a
`cargo-sources.json` (via
[flatpak-cargo-generator.py](https://github.com/flathub/flatpak-builder-tools))
and list it as a source, rather than committing a 500 MB `vendor/` directory.
That conversion is the remaining step before submitting to Flathub.
