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

## Build and install (local)

```bash
flatpak-builder --user --install --force-clean \
    --install-deps-from=flathub \
    build-dir flatpak/io.github.LyKhoris.Yunee.yml

flatpak run io.github.LyKhoris.Yunee
```

## Build a single-file bundle (for a release)

```bash
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

## Permissions

| Permission | Why |
|---|---|
| `--share=network` | Syncing with Canvas over HTTPS |
| `--socket=wayland`, `--socket=fallback-x11`, `--share=ipc`, `--device=dri` | Display |
| `--talk-name=org.freedesktop.notifications` | New-announcement notifications |
| `--talk-name=org.freedesktop.secrets` | The GNOME keyring, where the Canvas token is stored |
| `--filesystem=xdg-download` | Saving files downloaded from Canvas |

## Offline builds (Flathub)

This manifest fetches crates from crates.io during the build, which is fine for
local and GitHub releases. Submitting to Flathub requires vendoring the crates
into `flatpak/cargo-sources.json` (via
[flatpak-cargo-generator.py](https://github.com/flathub/flatpak-builder-tools))
and building with `CARGO_NET_OFFLINE=true`. That step is not done yet.
