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

## Publishing the update repository

An update is published with an explicit version:

```bash
gh workflow run publish.yml --ref main -f version=0.1.0-beta.7
```

`.github/workflows/publish.yml` builds the app, signs the OSTree repo, and
publishes it and the `.flatpakref` to `gh-pages`, served at
<https://lykhoris.github.io/Yunee/>. No GitHub Releases or git tags are created.
Pages hosts the update files; it does not run the desktop app.

GitHub Pages requires a public repository; `LyKhoris/Yunee` is public.

## The signing key

The repository is signed with a GPG key. The **secret** half lives in the
`FLATPAK_GPG_KEY` Actions secret (base64 of the ASCII-armored key); the **public**
half is embedded in the `.flatpakref`, so clients trust it on first install.

Generate it once — no passphrase, because CI imports it non-interactively:

```bash
gpg --batch --passphrase '' --quick-generate-key \
    "Yunee Flatpak Signing <email>" rsa4096 sign 0
gpg --export-secret-keys --armor <KEYID> | base64 -w0 \
    | gh secret set FLATPAK_GPG_KEY --repo LyKhoris/Yunee
```

**Back the secret up offline.** If it is lost you cannot sign updates, and every
user must re-add the remote because the remote's key changed. It has no expiry, so
it never needs rotating otherwise.

## Updates

Users install once from the `.flatpakref`:

```bash
flatpak install --from https://lykhoris.github.io/Yunee/io.github.LyKhoris.Yunee.flatpakref
```

and then update with `flatpak update` or GNOME Software's Updates page.

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
