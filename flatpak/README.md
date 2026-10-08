# Yunee — Flatpak packaging

Yunee ships as a Flatpak built on the GNOME runtime, with Rust provided by the
`org.freedesktop.Sdk.Extension.rust-stable` SDK extension. The manifest is
[`io.github.LyKhoris.Yunee.yml`](io.github.LyKhoris.Yunee.yml).

- **App id:** `io.github.LyKhoris.Yunee`
- **Binary:** `yunee`
- **Runtime:** `org.gnome.Platform//51` / `org.gnome.Sdk//51` (newest on Flathub).
  The development host is Fedora 44 with GTK 4.22 / libadwaita 1.9, i.e. the
  GNOME **50** runtime; if you need that exact baseline, change both
  `runtime-version` and `sdk` to `'50'` together in the manifest.

## 1. Prerequisite: generate `cargo-sources.json`

Flatpak builds run with **no network access**, so every crate in `Cargo.lock`
must be vendored ahead of time. `flatpak/cargo-sources.json` is produced by
[`flatpak-cargo-generator.py`](https://github.com/flatpak/flatpak-builder-tools/tree/master/cargo)
from that repository. It is **not** committed by hand — regenerate it whenever
`Cargo.lock` changes.

```bash
# one-time: get the generator
git clone https://github.com/flatpak/flatpak-builder-tools.git
cd flatpak-builder-tools/cargo

# needs Python 3.9+ with tomlkit + aiohttp (+ PyYAML for YAML output)
python3 flatpak-cargo-generator.py /home/lykhoris/Documents/clones/Yunee/Cargo.lock \
    -o /home/lykhoris/Documents/clones/Yunee/flatpak/cargo-sources.json
```

Run it from the repository root and write the output to
`flatpak/cargo-sources.json`, which is where the manifest's
`- cargo-sources.json` source-list entry looks for it. The generated file also
contains the cargo `config` that points cargo at the vendored crates; because the
manifest sets `CARGO_HOME=/run/build/yunee/cargo`, cargo picks it up
automatically and builds with `--offline`.

The manifest expects the file to exist — the build fails without it.

## 2. Build and install

Install the matching Rust SDK extension once (its branch must match the
`org.freedesktop.Sdk` branch that the GNOME runtime is based on):

```bash
flatpak install flathub org.freedesktop.Sdk.Extension.rust-stable
```

Then build the app from the repository root:

```bash
flatpak-builder --user --install --force-clean build-dir \
    flatpak/io.github.LyKhoris.Yunee.yml
```

If the runtime or SDK is not installed yet, let flatpak-builder fetch it from
Flathub:

```bash
flatpak-builder --user --install --force-clean \
    --install-deps-from=flathub \
    build-dir flatpak/io.github.LyKhoris.Yunee.yml
```

## 3. Run

```bash
flatpak run io.github.LyKhoris.Yunee
```

The app's data lives under `~/.var/app/io.github.LyKhoris.Yunee/data/yunee/`
(the sandboxed `$XDG_DATA_HOME`), containing `yunee.db` and downloaded files.
The Canvas token is stored in the GNOME keyring (Secret Service), with the
`0600` file fallback as described in `AGENTS.md`.

## 4. Publishing to Flathub (later)

This manifest is ready to submit once the app is stable:

1. Open a pull request against the [`flathub/flathub`](https://github.com/flathub/flathub)
   repository adding `io.github.LyKhoris.Yunee.yml` and the generated
   `cargo-sources.json` (Flathub requires the vendored sources to be committed,
   unlike a purely local build).
2. Keep `data/` out of `.gitignore`: the desktop file, AppStream metainfo, and
   SVG icon must be part of the repo so they can be installed into
   `/app/share/...` and discovered by Flathub and GNOME Software.
3. Validate before submitting:
   ```bash
   flatpak-builder-lint manifest flatpak/io.github.LyKhoris.Yunee.yml
   flatpak-builder-lint repo repo/
   appstreamcli validate data/io.github.LyKhoris.Yunee.metainfo.xml
   ```
   `flatpak-builder-lint` is strict about metainfo, screenshots, and the icon
   (`io.github.LyKhoris.Yunee.svg` at at least 128×128), so fix any errors it
   reports.
4. Tag a release that matches the `<release version="…">` entry in the
   metainfo, and keep `Cargo.lock` and `cargo-sources.json` in sync.
