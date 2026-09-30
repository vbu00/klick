//! Установщик kl!ck без окна: что стоит на компьютере, архив с программой, шаги установки
//! и удаления, системные мелочи. Окно и тихий режим — в `main.rs`.

#[cfg(windows)]
pub mod migrate;
#[cfg(windows)]
pub mod payload;
#[cfg(windows)]
pub mod plan;
#[cfg(windows)]
pub mod steps;
#[cfg(windows)]
pub mod webview2;
#[cfg(windows)]
pub mod win;
