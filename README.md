# GullDeb 🦆

[![Build](https://github.com/neverjoskiy/gulldeb/actions/workflows/build.yml/badge.svg)](https://github.com/neverjoskiy/gulldeb/actions/workflows/build.yml)
[![Release](https://img.shields.io/github/v/release/neverjoskiy/gulldeb?color=blue)](https://github.com/neverjoskiy/gulldeb/releases)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.75%2B-orange.svg)](https://www.rust-lang.org)
[![Platform](https://img.shields.io/badge/platform-Arch%20Linux%20%7C%20CachyOS-lightgrey.svg)](https://archlinux.org)

> **Convert Debian `.deb` packages into native Arch Linux `.pkg.tar.zst` packages — and install them with `pacman`.**
>
> No `makepkg`, no `debtap`, no GUI. Just one binary, a pretty terminal, and a double-click.

![GullDeb demo](docs/demo.svg)

---

## ✨ Features

- 📦 **One-command conversion** — `gulldeb package.deb` produces a valid `.pkg.tar.zst`
- 🎨 **Beautiful terminal UI** — step-by-step progress, spinners, progress bars, summary box
- 🔗 **Dependency mapping** — built-in Debian → Arch dictionary (openssl, gtk3, glibc, …)
- 🛡️ **Safe by default** — missing deps are warned about and skipped, never break install
- 🖱️ **Double-click install** — `Terminal=true` desktop entry opens a terminal on click
- ⚡ **Fast** — zstd level 19, real installed-size accounting
- 🐧 **CachyOS / Arch ready** — output is a normal pacman package

## 🚀 Quick install

```bash
curl -fsSL https://raw.githubusercontent.com/neverjoskiy/gulldeb/main/install.sh | bash
```

Or from a clone:

```bash
git clone https://github.com/neverjoskiy/gulldeb.git
cd gulldeb
./install.sh
```

The installer:
- downloads (or builds) the `gulldeb` binary into `~/.local/bin`
- adds `~/.local/bin` to your `PATH` (fish / zsh / bash detected)
- installs `gulldeb.desktop` and registers the `.deb` MIME association
- verifies the installation

## 🛠 Usage

```bash
# Convert + install
gulldeb /path/to/package.deb

# Convert only (no pacman)
gulldeb /path/to/package.deb --no-install
```

### Example output

```
╔════════════════════════════════════════════════════╗
║              GullDeb  •  .deb → .pkg.tar.zst       ║
╚════════════════════════════════════════════════════╝
  📦  /home/user/Downloads/example.deb

━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
  ➜  Step 1: Extracting .deb archive
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
   ⠋ Reading ar archive members...
   ✓  Extracted control.tar.* and data.tar.*
   ✓  control/ and data/ unpacked
...
╔════════════════════════════════════════════════════╗
║                  GullDeb Summary                   ║
╠════════════════════════════════════════════════════╣
║  📦 Package : example                              ║
║  🏷  Version : 1.2.3                               ║
║  🏗  Arch    : x86_64                              ║
║  🔗 Deps    : openssl, gtk3, glibc                 ║
║  💾 Archive : example-1.2.3-x86_64.pkg.tar.zst     ║
╚════════════════════════════════════════════════════╝
```

## 🖱 Double-click install

After `./install.sh`, double-clicking any `.deb` file in your file manager (Dolphin, Nautilus, Thunar, …) opens a terminal, runs GullDeb with progress, and asks for your `sudo` password — just like a native package.

## 🔧 How it works

1. **Extract** — reads the `.deb` with the `ar` crate, unpacks `control.tar.*` and `data.tar.*` (gz / xz / zst).
2. **Parse** — extracts `Package`, `Version`, `Architecture`, `Description`, `Depends` from `control`.
3. **Map deps** — Debian → Arch dictionary, strips version constraints `(>= …)` and alternatives `|`.
4. **Fix paths** — moves `usr/lib/*-linux-gnu/` → `usr/lib/`.
5. **Generate `.PKGINFO`** — with real installed size (fixed-point including `.PKGINFO` itself).
6. **Pack** — `.pkg.tar.zst` (zstd-19) next to the source `.deb`.
7. **Install** — `sudo pacman -U <archive>` (unless `--no-install`).

## 🏗 Build from source

```bash
git clone https://github.com/neverjoskiy/gulldeb.git
cd gulldeb
cargo build --release
# binary at target/release/gulldeb
```

CI builds happen on GitHub Actions — see [`.github/workflows`](.github/workflows).

## 📥 Releases

Pre-built binaries are published on the [Releases page](https://github.com/neverjoskiy/gulldeb/releases).
Each release ships `gulldeb` and a `gulldeb.sha256` checksum.

## 🤝 Contributing

PRs welcome. Please run `cargo fmt` and `cargo clippy` before submitting.

## 📄 License

MIT — see [LICENSE](LICENSE).
SH
echo done