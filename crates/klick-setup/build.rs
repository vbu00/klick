//! Сборка установщика: архив с программой вшивается в exe, манифест просит права администратора.
//!
//! В архив идут `klick.exe` и `klick-service.exe` из той же папки сборки (`target/debug` или
//! `target/release`) и папка `resources`. Поэтому сначала собираются окно и служба:
//! `cargo build -p klick-ui -p klick-service`, затем `cargo build -p klick-setup`.
//! Папку с exe можно задать явно: `KLICK_PAYLOAD_BIN`.

use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use zip::write::SimpleFileOptions;

fn main() {
    let manifest = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let app = manifest.join("../..").canonicalize().expect("папка app");
    let profile = std::env::var("PROFILE").unwrap_or_else(|_| "debug".into());
    let target = std::env::var_os("CARGO_TARGET_DIR").map(PathBuf::from).unwrap_or_else(|| app.join("target"));
    let bin = std::env::var_os("KLICK_PAYLOAD_BIN").map(PathBuf::from).unwrap_or_else(|| target.join(&profile));
    println!("cargo:rerun-if-env-changed=KLICK_PAYLOAD_BIN");

    let out = PathBuf::from(std::env::var("OUT_DIR").unwrap()).join("payload.zip");
    let mut zip = zip::ZipWriter::new(File::create(&out).expect("payload.zip"));
    let opts = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated).large_file(true);
    let mut total: u64 = 0;

    for exe in ["klick.exe", "klick-service.exe"] {
        let path = bin.join(exe);
        if !path.exists() {
            panic!("нет {} — сначала собери окно и службу: cargo build -p klick-ui -p klick-service (профиль {profile})", path.display());
        }
        total += add(&mut zip, &path, exe, opts);
    }
    total += add_dir(&mut zip, &app.join("resources"), "resources", opts);
    zip.finish().expect("архив");

    // Сколько места нужно на диске — для экрана «Добро пожаловать».
    println!("cargo:rustc-env=KLICK_PAYLOAD_BYTES={total}");
    // Версия ядра — для строки «Ядро mihomo v…» на экране установки.
    println!("cargo:rustc-env=KLICK_CORE_VERSION={}", core_version(&app.join("resources").join("core").join("mihomo.exe")));

    let attrs = tauri_build::Attributes::new().windows_attributes(tauri_build::WindowsAttributes::new().app_manifest(include_str!("setup.manifest")));
    tauri_build::try_build(attrs).expect("tauri-build");
}

/// `mihomo -v` печатает «Mihomo Meta v1.19.31 windows amd64 …».
fn core_version(exe: &Path) -> String {
    let out = std::process::Command::new(exe).arg("-v").output().map(|o| String::from_utf8_lossy(&o.stdout).into_owned()).unwrap_or_default();
    out.split_whitespace()
        .find(|w| w.starts_with('v') && w[1..].starts_with(|c: char| c.is_ascii_digit()))
        .unwrap_or_default()
        .to_string()
}

fn add(zip: &mut zip::ZipWriter<File>, path: &Path, name: &str, opts: SimpleFileOptions) -> u64 {
    println!("cargo:rerun-if-changed={}", path.display());
    let mut data = Vec::new();
    File::open(path).and_then(|mut f| f.read_to_end(&mut data)).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    zip.start_file(name, opts).expect("zip");
    zip.write_all(&data).expect("zip");
    data.len() as u64
}

fn add_dir(zip: &mut zip::ZipWriter<File>, dir: &Path, prefix: &str, opts: SimpleFileOptions) -> u64 {
    let mut total = 0;
    let mut entries: Vec<_> = std::fs::read_dir(dir).unwrap_or_else(|e| panic!("{}: {e}", dir.display())).flatten().collect();
    entries.sort_by_key(|e| e.file_name());
    for e in entries {
        let path = e.path();
        let name = format!("{prefix}/{}", e.file_name().to_string_lossy());
        if path.is_dir() {
            total += add_dir(zip, &path, &name, opts);
        } else {
            total += add(zip, &path, &name, opts);
        }
    }
    total
}
