//! Ссылки `klick://add?url=…` со страницы подписки: разбор и проверки — в `klick_core::deeplink`.
//! Ссылка никуда не пишется: токен подписки в ней — секрет. Страница забирает её сама
//! (`take_pending_link`): при холодном старте событие пришло бы раньше, чем страница начала слушать.

use klick_core::deeplink::{parse, DeepLink};
use serde::Serialize;
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager, State};

#[derive(Clone, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Pending {
    Add { url: String, name: Option<String> },
    /// Ссылка не прошла проверки: окно скажет «Ссылка не подходит», без её текста.
    Invalid,
}

#[derive(Default)]
pub struct PendingLink(Mutex<Option<Pending>>);

/// Ссылка пришла (из аргументов этого процесса или от второй копии): открыть «Добавить».
/// Добавляет человек сам, кнопкой, — чужая страница не подсунет подписку молча.
pub fn open(app: &AppHandle, raw: &str) {
    let pending = match parse(raw, cfg!(debug_assertions)) {
        Ok(DeepLink::Add { url, name }) => Pending::Add { url, name },
        Ok(DeepLink::Ignore) => return,
        Err(_) => Pending::Invalid,
    };
    if let Some(state) = app.try_state::<PendingLink>() {
        *state.0.lock().unwrap_or_else(|e| e.into_inner()) = Some(pending);
    }
    crate::show_main(app);
    let _ = app.emit_to("main", "klick://add-link", ());
}

#[tauri::command]
pub fn take_pending_link(state: State<'_, PendingLink>) -> Option<Pending> {
    state.0.lock().unwrap_or_else(|e| e.into_inner()).take()
}
