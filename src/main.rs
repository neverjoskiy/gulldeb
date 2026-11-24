use anyhow::{anyhow, bail, Result};
use clap::Parser;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Parser, Debug)]
#[command(name = "gulldeb", version, about = "Convert a Debian .deb package into an Arch Linux .pkg.tar.zst package")]
struct Cli {
    /// Path to the .deb file
    deb: PathBuf,
    /// Only convert; do not run pacman -U
    #[arg(long = "no-install")]
    no_install: bool,
}

const DEP_MAP: &[(&str, &str)] = &[
    ("libssl3", "openssl"),
    ("libssl1.1", "openssl"),
    ("libgtk-3-0", "gtk3"),
    ("libgtk-4-1", "gtk4"),
    ("libasound2", "alsa-lib"),
    ("libnss3", "nss"),
    ("libxss1", "libxss"),
    ("libgbm1", "mesa"),
    ("zlib1g", "zlib"),
    ("libc6", "glibc"),
    ("libstdc++6", "gcc-libs"),
    ("libgcc-s1", "gcc-libs"),
    ("libcurl4", "curl"),
    ("libpng16-16", "libpng"),
    ("libjpeg-turbo8", "libjpeg-turbo"),
    ("libfreetype6", "freetype2"),
    ("libfontconfig1", "fontconfig"),
    ("libx11-6", "libx11"),
    ("libxext6", "libxext"),
    ("libxrender1", "libxrender"),
    ("libxi6", "libxi"),
    ("libxrandr2", "libxrandr"),
    ("libxfixes3", "libxfixes"),
    ("libxdamage1", "libxdamage"),
    ("libxcomposite1", "libxcomposite"),
    ("libxcursor1", "libxcursor"),
    ("libxinerama1", "libxinerama"),
    ("libdbus-1-3", "dbus"),
    ("libexpat1", "expat"),
    ("libuuid1", "util-linux-libs"),
    ("libpcre3", "pcre"),
    ("libselinux1", "libselinux"),
    ("libsystemd0", "systemd-libs"),
    ("libudev1", "systemd-libs"),
];

fn main() -> Result<()> {
    let cli = Cli::parse();

    let deb = &cli.deb;
    if !deb.is_file() {
        bail!("input file not found: {}", deb.display());
    }

    let tmp = tempfile::tempdir()?;
    let control_dir = tmp.path().join("control");
    let data_dir = tmp.path().join("data");
    fs::create_dir_all(&control_dir)?;
    fs::create_dir_all(&data_dir)?;

    println!("GullDeb: extracting {}", deb.display());
    extract_deb(deb, &control_dir, &data_dir)?;

    let control_path = find_file(&control_dir, "control")
        .ok_or_else(|| anyhow!("control file not found in .deb"))?;
    let control_text = fs::read_to_string(&control_path)?;
    let meta = parse_control(&control_text);

    let pkgname = meta
        .get("Package")
        .cloned()
        .unwrap_or_else(|| "unknown".to_string());
    let raw_version = meta
        .get("Version")
        .cloned()
        .unwrap_or_else(|| "1.0".to_string());
    let pkgver = sanitize_version(&raw_version);
    let arch_raw = meta
        .get("Architecture")
        .cloned()
        .unwrap_or_else(|| "amd64".to_string());
    let arch = map_arch(&arch_raw);
    let desc = meta.get("Description").cloned().unwrap_or_default();
    let pkgdesc = desc.lines().next().unwrap_or("").to_string();
    let url = meta.get("Homepage").cloned().unwrap_or_else(|| "https://github.com/neverjoskiy/gulldeb".to_string());
    let depends_str = meta.get("Depends").cloned().unwrap_or_default();

    let (found_deps, skipped_deps) = map_deps(&depends_str);

    // Fix library paths: usr/lib/x86_64-linux-gnu -> usr/lib
    let lib_src = data_dir.join("usr/lib/x86_64-linux-gnu");
    let lib_dst = data_dir.join("usr/lib");
    move_dir_contents(&lib_src, &lib_dst)?;
    let _ = fs::remove_dir(&lib_src);
    // Also handle aarch64 variant just in case
    let lib_src2 = data_dir.join("usr/lib/aarch64-linux-gnu");
    move_dir_contents(&lib_src2, &lib_dst)?;
    let _ = fs::remove_dir(&lib_src2);

    let pkg_root = &data_dir;

    let builddate = chrono::Utc::now().timestamp();

    // Fixed-point size computation including .PKGINFO itself.
    let mut size: u64 = 0;
    for _ in 0..6 {
        let content = generate_pkginfo(&pkgname, &pkgver, &pkgdesc, &url, &arch, builddate, size, &found_deps);
        fs::write(pkg_root.join(".PKGINFO"), &content)?;
        size = dir_size(pkg_root);
    }
    let content = generate_pkginfo(&pkgname, &pkgver, &pkgdesc, &url, &arch, builddate, size, &found_deps);
    fs::write(pkg_root.join(".PKGINFO"), &content)?;

    let out_dir = deb.parent().unwrap_or_else(|| Path::new("."));
    let archive_name = format!("{}-{}-{}.pkg.tar.zst", pkgname, pkgver, arch);
    let archive_path = out_dir.join(&archive_name);

    println!("GullDeb: creating {}", archive_path.display());
    create_archive(pkg_root, &archive_path)?;

    println!();
    println!("========== GullDeb Summary ==========");
    println!("  Package : {}", pkgname);
    println!("  Version : {}", pkgver);
    println!("  Arch    : {}", arch);
    println!("  Desc    : {}", pkgdesc);
    println!("  Found deps   : {}", found_deps.join(", "));
    println!("  Skipped deps : {}", skipped_deps.join(", "));
    println!("  Archive : {}", archive_path.display());
    println!("=====================================");

    if !cli.no_install {
        println!("GullDeb: installing via sudo pacman -U ...");
        let status = Command::new("sudo")
            .arg("pacman")
            .arg("-U")
            .arg(&archive_path)
            .status()?;
        if !status.success() {
            bail!("pacman installation failed with status {}", status);
        }
    } else {
        println!("GullDeb: --no-install set, skipping installation.");
    }

    Ok(())
}

fn extract_deb(deb: &Path, control_dir: &Path, data_dir: &Path) -> Result<()> {
    let file = fs::File::open(deb)?;
    let mut archive = ar::Archive::new(file);
    let entries = archive.entries()?;
    for entry_res in entries {
        let mut entry = entry_res?;
        let raw_name = String::from_utf8_lossy(entry.header().name()).to_string();
        let name = raw_name.trim_matches(|c: char| c.is_whitespace() || c == '/').to_string();

        let mut buf = Vec::new();
        entry.read_to_end(&mut buf)?;

        if name.starts_with("control.tar") {
            unpack_tar_bytes(&buf, &name, control_dir)?;
        } else if name.starts_with("data.tar") {
            unpack_tar_bytes(&buf, &name, data_dir)?;
        }
        // ignore other members (debian-binary, etc.)
    }
    Ok(())
}

fn unpack_tar_bytes(buf: &[u8], name: &str, dest: &Path) -> Result<()> {
    let cursor = std::io::Cursor::new(buf);
    if name.ends_with(".gz") {
        let gz = flate2::read::GzDecoder::new(cursor);
        let mut tar_archive = tar::Archive::new(gz);
        tar_archive.unpack(dest)?;
    } else if name.ends_with(".xz") {
        let xz = xz2::read::XzDecoder::new(cursor);
        let mut tar_archive = tar::Archive::new(xz);
        tar_archive.unpack(dest)?;
    } else if name.ends_with(".zst") || name.ends_with(".zstd") {
        let zst = zstd::stream::read::Decoder::new(cursor)?;
        let mut tar_archive = tar::Archive::new(zst);
        tar_archive.unpack(dest)?;
    } else {
        let mut tar_archive = tar::Archive::new(cursor);
        tar_archive.unpack(dest)?;
    }
    Ok(())
}

fn parse_control(text: &str) -> std::collections::BTreeMap<String, String> {
    let mut map = std::collections::BTreeMap::new();
    let mut current_key: Option<String> = None;
    for line in text.lines() {
        if line.starts_with(' ') || line.starts_with('\t') {
            if let Some(k) = &current_key {
                let v = map.get_mut(k).unwrap();
                v.push('\n');
                v.push_str(line.trim());
            }
        } else if let Some((k, v)) = line.split_once(':') {
            let key = k.trim().to_string();
            let val = v.trim().to_string();
            map.insert(key.clone(), val);
            current_key = Some(key);
        } else if line.trim().is_empty() {
            current_key = None;
        }
    }
    map
}

fn sanitize_version(v: &str) -> String {
    v.trim().replace('-', ".").replace(':', ".")
}

fn map_arch(a: &str) -> String {
    match a.trim() {
        "amd64" => "x86_64".to_string(),
        "arm64" => "aarch64".to_string(),
        "all" => "any".to_string(),
        "i386" => "i686".to_string(),
        other => other.to_string(),
    }
}

fn map_deps(depends: &str) -> (Vec<String>, Vec<String>) {
    let mut found = Vec::new();
    let mut skipped = Vec::new();
    if depends.trim().is_empty() {
        return (found, skipped);
    }
    for part in depends.split(',') {
        // first alternative
        let alt = part.split('|').next().unwrap_or("").trim();
        // strip version constraints in parentheses
        let name_part = alt.split('(').next().unwrap_or(alt).trim();
        // take first token
        let name = name_part.split_whitespace().next().unwrap_or("").trim();
        let name = name.split(':').next().unwrap_or(name).trim();
        if name.is_empty() {
            continue;
        }
        let mapped = DEP_MAP
            .iter()
            .find(|(d, _)| *d == name)
            .map(|(_, a)| a.to_string())
            .unwrap_or_else(|| name.to_string());

        if pkg_available(&mapped) {
            if !found.contains(&mapped) {
                found.push(mapped);
            }
        } else {
            eprintln!("Warning: dependency '{}' not found in Arch system/repos, skipping", mapped);
            if !skipped.contains(&mapped) {
                skipped.push(mapped);
            }
        }
    }
    (found, skipped)
}

fn pkg_available(pkg: &str) -> bool {
    if let Ok(out) = Command::new("pacman").args(["-Qq", pkg]).output() {
        if out.status.success() {
            return true;
        }
    }
    if let Ok(out) = Command::new("pacman").args(["-Si", pkg]).output() {
        if out.status.success() {
            return true;
        }
    }
    false
}

fn move_dir_contents(src: &Path, dst: &Path) -> Result<()> {
    if !src.exists() {
        return Ok(());
    }
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let from = entry.path();
        let to = dst.join(entry.file_name());
        if from.is_dir() && !from.is_symlink() {
            fs::create_dir_all(&to)?;
            move_dir_contents(&from, &to)?;
            let _ = fs::remove_dir(&from);
        } else {
            if to.exists() {
                if to.is_dir() && !to.is_symlink() {
                    let _ = fs::remove_dir_all(&to);
                } else {
                    let _ = fs::remove_file(&to);
                }
            }
            fs::rename(&from, &to)?;
        }
    }
    Ok(())
}

fn find_file(root: &Path, name: &str) -> Option<PathBuf> {
    if root.is_file() {
        if root.file_name().map(|n| n == name).unwrap_or(false) {
            return Some(root.to_path_buf());
        }
        return None;
    }
    if let Ok(entries) = fs::read_dir(root) {
        for e in entries.flatten() {
            let p = e.path();
            if p.is_dir() {
                if let Some(f) = find_file(&p, name) {
                    return Some(f);
                }
            } else if p.file_name().map(|n| n == name).unwrap_or(false) {
                return Some(p);
            }
        }
    }
    None
}

fn generate_pkginfo(
    pkgname: &str,
    pkgver: &str,
    pkgdesc: &str,
    url: &str,
    arch: &str,
    builddate: i64,
    size: u64,
    deps: &[String],
) -> String {
    let mut s = String::new();
    s.push_str("# Generated by GullDeb\n");
    s.push_str(&format!("pkgname = {}\n", pkgname));
    s.push_str(&format!("pkgver = {}\n", pkgver));
    s.push_str(&format!("pkgdesc = {}\n", pkgdesc));
    s.push_str(&format!("url = {}\n", url));
    s.push_str(&format!("builddate = {}\n", builddate));
    s.push_str("packager = GullDeb\n");
    s.push_str(&format!("size = {}\n", size));
    s.push_str(&format!("arch = {}\n", arch));
    for d in deps {
        s.push_str(&format!("depend = {}\n", d));
    }
    s
}

fn dir_size(path: &Path) -> u64 {
    let mut total = 0u64;
    if let Ok(entries) = fs::read_dir(path) {
        for e in entries.flatten() {
            let p = e.path();
            if p.is_symlink() {
                if let Ok(m) = fs::symlink_metadata(&p) {
                    total += m.len();
                }
            } else if p.is_dir() {
                total += dir_size(&p);
            } else if let Ok(m) = fs::metadata(&p) {
                total += m.len();
            }
        }
    }
    total
}

fn collect_files(root: &Path, current: &Path, out: &mut Vec<PathBuf>) {
    if let Ok(entries) = fs::read_dir(current) {
        for e in entries.flatten() {
            let p = e.path();
            if let Ok(rel) = p.strip_prefix(root) {
                out.push(rel.to_path_buf());
            }
            if p.is_dir() && !p.is_symlink() {
                collect_files(root, &p, out);
            }
        }
    }
}

fn create_archive(root: &Path, out: &Path) -> Result<()> {
    let file = fs::File::create(out)?;
    let enc = zstd::stream::write::Encoder::new(file, 19)?;
    let mut builder = tar::Builder::new(enc);

    let mut entries: Vec<PathBuf> = Vec::new();
    collect_files(root, root, &mut entries);
    entries.sort();
    if let Some(pos) = entries.iter().position(|p| p == Path::new(".PKGINFO")) {
        let p = entries.remove(pos);
        entries.insert(0, p);
    }

    for rel in entries {
        let full = root.join(&rel);
        let rel_str = rel.to_string_lossy().to_string();
        if full.is_symlink() {
            let target = fs::read_link(&full)?;
            let mut header = tar::Header::new_gnu();
            header.set_path(&rel_str)?;
            header.set_entry_type(tar::EntryType::Symlink);
            header.set_size(0);
            header.set_mode(0o777);
            header.set_mtime(0);
            header.set_uid(0);
            header.set_gid(0);
            header.set_uname("root");
            header.set_gname("root");
            header.set_cksum();
            builder.append_link(&mut header, target)?;
        } else if full.is_dir() {
            let mut header = tar::Header::new_gnu();
            header.set_path(format!("{}/", rel_str))?;
            header.set_entry_type(tar::EntryType::Directory);
            header.set_size(0);
            header.set_mode(0o755);
            header.set_mtime(0);
            header.set_uid(0);
            header.set_gid(0);
            header.set_uname("root");
            header.set_gname("root");
            header.set_cksum();
            builder.append_data(&mut header, std::io::empty())?;
        } else {
            let meta = fs::metadata(&full)?;
            let mode = meta.permissions().mode();
            let size = meta.len();
            let mut header = tar::Header::new_gnu();
            header.set_path(&rel_str)?;
            header.set_entry_type(tar::EntryType::Regular);
            header.set_size(size);
            header.set_mode(mode);
            header.set_mtime(0);
            header.set_uid(0);
            header.set_gid(0);
            header.set_uname("root");
            header.set_gname("root");
            header.set_cksum();
            let mut f = fs::File::open(&full)?;
            builder.append_data(&mut header, &mut f)?;
        }
    }

    let enc = builder.into_inner()?;
    enc.finish()?;
    Ok(())
}
