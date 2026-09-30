//! Системный прокси на macOS ставит и снимает служба: настройки сети там общие для всех
//! пользователей и меняются только с правами администратора, а служба работает от root.
//! Окну делать нечего — те же функции, что у Windows, чтобы мост к службе был один.

use serde::Deserialize;

/// Каким прокси должен быть; окно на macOS только читает его из состояния службы.
#[allow(dead_code)]
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct Spec {
    pub host: String,
    pub port: u16,
    pub bypass: Vec<String>,
}

pub fn reconcile(_desired: Option<Spec>) {}

pub fn service_lost() {}

pub fn clear_ours() {}
