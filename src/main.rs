use anyhow::{anyhow, bail, Result};
use clap::Parser;
use indicatif::{ProgressBar, ProgressStyle};
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::thread;
use std::time::Duration;

#[derive(Parser, Debug)]
#[command(name = "gulldeb", version, about = "Convert a Debian .deb package into an Arch Linux .pkg.tar.zst package")]
struct Cli {
    /// Path to the .deb file
    deb: PathBuf,
    /// Only convert; do not run pacman -U
    #[arg(long = "no-install")]
    no_install: bool,
    /// Skip dependency availability checks (faster)
    #[arg(long = "skip-deps")]
    skip_deps: bool,
    /// zstd compression level (1-19, lower = faster). Default 6
    #[arg(long = "compression-level", default_value_t = 6)]
    compression_level: i32,
    /// Keep the .pkg.tar.zst archive after successful installation
    #[arg(long = "keep-archive")]
    keep_archive: bool,
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
    ("libatk1.0-0", "atk"),
    ("libatk-bridge2.0-0", "at-spi2-core"),
    ("libatspi2.0-0", "at-spi2-core"),
    ("libcairo2", "cairo"),
    ("libcups2", "libcups"),
    ("libcurl3-gnutls", "curl"),
    ("libglib2.0-0", "glib2"),
    ("libnspr4", "nspr"),
    ("libpango-1.0-0", "pango"),
    ("libxcb1", "libxcb"),
    ("libxkbcommon0", "libxkbcommon"),
    ("libxkbfile1", "libxkbfile"),
    ("libgtk-3-0", "gtk3"),
    ("libgtk-4-1", "gtk4"),
    ("libnss3", "nss"),
    ("libasound2", "alsa-lib"),
    ("libxss1", "libxss"),
    ("libgbm1", "mesa"),
    ("zlib1g", "zlib"),
    ("libstdc++6", "gcc-libs"),
    ("libgcc-s1", "gcc-libs"),
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
];

fn step(n: u8, title: &str) {
    println!();
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("  ➜  Step {}: {}", n, title);
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
}

fn ok(msg: &str) {
    println!("   ✓  {}", msg);
}

fn warn(msg: &str) {
    eprintln!("   ⚠  {}", msg);
}

fn spinner(msg: &str) -> ProgressBar {
    let sp = ProgressBar::new_spinner();
    sp.set_style(ProgressStyle::with_template("{spinner:.cyan} {msg}").unwrap());
    sp.set_message(msg.to_string());
    sp.enable_steady_tick(Duration::from_millis(80));
    sp
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    let deb = &cli.deb;
    if !deb.is_file() {
        bail!("input file not found: {}", deb.display());
    }

    println!();
    println!("╔════════════════════════════════════════════════════╗");
    println!("║              GullDeb  •  .deb → .pkg.tar.zst       ║");
    println!("╚════════════════════════════════════════════════════╝");
    println!("  📦  {}", deb.display());

    let tmp = tempfile::tempdir()?;
    let control_dir = tmp.path().join("control");
    let data_dir = tmp.path().join("data");
    fs::create_dir_all(&control_dir)?;
    fs::create_dir_all(&data_dir)?;

    step(1, "Extracting .deb archive");
    let sp = spinner("Reading ar archive members...");
    extract_deb(deb, &control_dir, &data_dir)?;
    sp.finish_with_message("Extracted control.tar.* and data.tar.*");
    ok("control/ and data/ unpacked");

    step(2, "Parsing package metadata");
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
    let url = meta
        .get("Homepage")
        .cloned()
        .unwrap_or_else(|| "https://github.com/neverjoskiy/gulldeb".to_string());
    let depends_str = meta.get("Depends").cloned().unwrap_or_default();

    ok(&format!("Package   : {}", pkgname));
    ok(&format!("Version   : {}  →  {}", raw_version, pkgver));
    ok(&format!("Arch      : {}  →  {}", arch_raw, arch));
    ok(&format!("Desc      : {}", pkgdesc));

    step(3, "Resolving dependencies");
    let (found_deps, skipped_deps) = map_deps(&depends_str, cli.skip_deps);
    if found_deps.is_empty() && skipped_deps.is_empty() {
        ok("No dependencies declared");
    } else {
        ok(&format!("Found   : {}", if found_deps.is_empty() { "(none)".to_string() } else { found_deps.join(", ") }));
        if !skipped_deps.is_empty() {
            warn(&format!("Skipped : {}", skipped_deps.join(", ")));
        }
    }

    step(4, "Fixing library paths");
    let lib_src = data_dir.join("usr/lib/x86_64-linux-gnu");
    let lib_dst = data_dir.join("usr/lib");
    move_dir_contents(&lib_src, &lib_dst)?;
    let _ = fs::remove_dir(&lib_src);
    let lib_src2 = data_dir.join("usr/lib/aarch64-linux-gnu");
    move_dir_contents(&lib_src2, &lib_dst)?;
    let _ = fs::remove_dir(&lib_src2);
    ok("usr/lib/*-linux-gnu → usr/lib");

    let pkg_root = &data_dir;
    let builddate = chrono::Utc::now().timestamp();

    step(5, "Generating .PKGINFO");
    let sp = spinner("Computing installed size...");
    let mut size: u64 = 0;
    for _ in 0..6 {
        let content = generate_pkginfo(&pkgname, &pkgver, &pkgdesc, &url, &arch, builddate, size, &found_deps);
        fs::write(pkg_root.join(".PKGINFO"), &content)?;
        size = dir_size(pkg_root);
    }
    let content = generate_pkginfo(&pkgname, &pkgver, &pkgdesc, &url, &arch, builddate, size, &found_deps);
    fs::write(pkg_root.join(".PKGINFO"), &content)?;
    let m = format!("size = {} bytes", size);
    sp.finish_with_message(m);
    ok(".PKGINFO written");

    step(6, "Building .pkg.tar.zst archive");
    let out_dir = deb.parent().unwrap_or_else(|| Path::new("."));
    let archive_name = format!("{}-{}-{}.pkg.tar.zst", pkgname, pkgver, arch);
    let archive_path = out_dir.join(&archive_name);

    let sp = spinner(&format!("Compressing → {}", archive_name));
    create_archive(pkg_root, &archive_path, cli.compression_level)?;
    let m = format!("Created {}", archive_path.display());
    sp.finish_with_message(m);

    println!();
    println!("╔════════════════════════════════════════════════════╗");
    println!("║                  GullDeb Summary                   ║");
    println!("╠════════════════════════════════════════════════════╣");
    println!("║  📦 Package : {:<38}║", pkgname);
    println!("║  🏷  Version : {:<38}║", pkgver);
    println!("║  🏗  Arch    : {:<38}║", arch);
    println!("║  📝 Desc    : {:<38}║", truncate(&pkgdesc, 38));
    println!("║  🔗 Deps    : {:<38}║", truncate(&found_deps.join(", "), 38));
    if !skipped_deps.is_empty() {
        println!("║  ⚠  Skipped : {:<38}║", truncate(&skipped_deps.join(", "), 38));
    }
    println!("║  💾 Archive : {:<38}║", truncate(&archive_path.display().to_string(), 38));
    println!("╚════════════════════════════════════════════════════╝");

    if !cli.no_install {
        step(7, "Installing via pacman");
        println!("   🔐  sudo pacman -U {}", archive_path.display());
        let status = Command::new("sudo")
            .arg("pacman")
            .arg("-U")
            .arg(&archive_path)
            .status()?;
        if !status.success() {
            bail!("pacman installation failed with status {}", status);
        }
        ok("Installation completed");
        if !cli.keep_archive {
            let _ = fs::remove_file(&archive_path);
            ok(&format!("Removed archive {}", archive_path.display()));
        }
    } else {
        println!();
        println!("   ℹ️   --no-install set, skipping pacman installation.");
    }

    println!();
    println!("   ✨  Done. Thank you for using GullDeb!");
    Ok(())
}

fn truncate(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        format!("{:<width$}", s, width = n)
    } else {
        let mut out: String = s.chars().take(n.saturating_sub(1)).collect();
        out.push('…');
        out
    }
}

fn extract_deb(deb: &Path, control_dir: &Path, data_dir: &Path) -> Result<()> {
    let file = fs::File::open(deb)?;
    let mut archive = ar::Archive::new(file);
    while let Some(entry_res) = archive.next_entry() {
        let mut entry = entry_res?;
        let raw_name = entry.header().identifier().to_string();
        let name = raw_name.trim_matches(|c: char| c.is_whitespace() || c == '/').to_string();

        let mut buf = Vec::new();
        entry.read_to_end(&mut buf)?;

        if name.starts_with("control.tar") {
            unpack_tar_bytes(&buf, &name, control_dir)?;
        } else if name.starts_with("data.tar") {
            unpack_tar_bytes(&buf, &name, data_dir)?;
        }
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
    let mut map: std::collections::BTreeMap<String, String> = std::collections::BTreeMap::new();
    let mut current_key: Option<String> = None;
    for line in text.lines() {
        if line.starts_with(' ') || line.starts_with('\t') {
            if let Some(k) = &current_key {
                if let Some(v) = map.get_mut(k) {
                    v.push('\n');
                    v.push_str(line.trim());
                }
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
    // pacman requires pkgver to contain a package release: "<version>-<pkgrel>"
    // (pkgrel must be an integer). Debian revisions use '-', which we flatten
    // into '.', so append a default pkgrel of 1 when none is present.
    let base = v.trim().replace('-', ".").replace(':', ".");
    if base.contains('-') {
        base
    } else {
        format!("{}-1", base)
    }
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

fn map_deps(depends: &str, skip_deps: bool) -> (Vec<String>, Vec<String>) {
    let mut found = Vec::new();
    let mut skipped = Vec::new();
    if depends.trim().is_empty() {
        return (found, skipped);
    }
    // First pass: collect unique mapped deps
    let mut mapped_list: Vec<String> = Vec::new();
    for part in depends.split(',') {
        let alt = part.split('|').next().unwrap_or("").trim();
        let name_part = alt.split('(').next().unwrap_or(alt).trim();
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
        if !mapped_list.contains(&mapped) {
            mapped_list.push(mapped);
        }
    }

    if skip_deps {
        found.extend(mapped_list);
        return (found, skipped);
    }

    // Batch check availability in a single pacman invocation (much faster)
    let sp = spinner(&format!("Checking {} dependencies...", mapped_list.len()));
    let available_set = batch_available(&mapped_list);
    sp.finish_with_message("done");
    for mapped in mapped_list {
        if available_set.contains(&mapped) {
            ok(&format!("{}  ✓", mapped));
            if !found.contains(&mapped) {
                found.push(mapped);
            }
        } else {
            warn(&format!("dependency '{}' not found, skipping", mapped));
            if !skipped.contains(&mapped) {
                skipped.push(mapped);
            }
        }
    }
    (found, skipped)
}

fn batch_available(pkgs: &[String]) -> std::collections::HashSet<String> {
    let mut set = std::collections::HashSet::new();
    if pkgs.is_empty() {
        return set;
    }
    // First: locally installed (fast, offline)
    let mut args = vec!["-Qq".to_string()];
    for p in pkgs { args.push(p.clone()); }
    if let Ok(out) = Command::new("pacman").args(&args).output() {
        for line in String::from_utf8_lossy(&out.stdout).lines() {
            let name = line.trim();
            if !name.is_empty() {
                set.insert(name.to_string());
            }
        }
    }
    let remaining: Vec<&String> = pkgs.iter().filter(|p| !set.contains(*p)).collect();
    if !remaining.is_empty() {
        // One pacman -Si call for the rest (sync db)
        let mut args2 = vec!["-Si".to_string()];
        for p in &remaining { args2.push(p.to_string()); }
        if let Ok(out) = Command::new("pacman").args(&args2).output() {
            let text = String::from_utf8_lossy(&out.stdout);
            for line in text.lines() {
                let t = line.trim();
                if let Some(rest) = t.strip_prefix("Name") {
                    let name = rest.trim_start_matches(|c: char| c == ' ' || c == ':').trim();
                    let name = name.split_whitespace().next().unwrap_or("").to_string();
                    if !name.is_empty() && remaining.iter().any(|p| **p == name) {
                        set.insert(name);
                    }
                }
            }
        }
    }
    set
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

fn create_archive(root: &Path, out: &Path, level: i32) -> Result<()> {
    let file = fs::File::create(out)?;
    let lvl = level.clamp(1, 19);
    let enc = zstd::stream::write::Encoder::new(file, lvl)?;
    let mut builder = tar::Builder::new(enc);

    let mut entries: Vec<PathBuf> = Vec::new();
    collect_files(root, root, &mut entries);
    entries.sort();
    if let Some(pos) = entries.iter().position(|p| p == Path::new(".PKGINFO")) {
        let p = entries.remove(pos);
        entries.insert(0, p);
    }

    let pb = ProgressBar::new(entries.len() as u64);
    pb.set_style(ProgressStyle::with_template("   {bar:30.cyan/blue} {pos}/{len} {msg}").unwrap());
    for rel in entries {
        let full = root.join(&rel);
        let rel_str = rel.to_string_lossy().to_string();
        pb.set_message(rel_str.clone());
        builder.append_path_with_name(&full, &rel_str)?;
        pb.inc(1);
    }
    pb.finish_with_message("archive entries added");

    let enc = builder.into_inner()?;
    enc.finish()?;
    Ok(())
}
