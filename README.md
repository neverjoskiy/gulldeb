# GullDeb

Convert a Debian `.deb` package into a native Arch Linux `.pkg.tar.zst` package and install it with `pacman`.

## Usage

```sh
gulldeb /path/to/package.deb
gulldeb /path/to/package.deb --no-install   # convert only
```

Double-clicking a `.deb` file in a file manager opens GullDeb in a terminal (via the
`gulldeb.desktop` entry) so you can watch progress and enter your password.

## Install

```sh
./install.sh
```

Builds the release binary, copies it to `~/.local/bin/gulldeb`, installs the desktop
entry, and registers the MIME association for `.deb` files.

## Build (CI)

The project is built on GitHub Actions (see `.github/workflows/build.yml`).
