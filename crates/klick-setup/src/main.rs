//! Установщик kl!ck для Windows (см. `app.rs`). На macOS kl!ck ставится пакетом .pkg:
//! `scripts/macos/build.sh`, служба регистрируется в launchd командой `klick-service install`.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[cfg(windows)]
mod app;

#[cfg(windows)]
fn main() {
    app::main()
}

#[cfg(not(windows))]
fn main() {
    eprintln!("klick-setup — установщик для Windows. На macOS: scripts/macos/build.sh собирает kl!ck.app и kl!ck.pkg");
    std::process::exit(2);
}
