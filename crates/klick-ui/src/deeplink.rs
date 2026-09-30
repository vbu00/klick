//! Ссылка `klick://add` со страницы подписки: kl!ck открывается на экране «Добавить» с уже вставленной
//! подпиской, добавляет её человек своей кнопкой. Разбор и проверки — `klick_core::deeplink`.
//!
//! Ссылка сначала кладётся в состояние, страница окна забирает её сама (`take_pending_link`): при
//! холодном старте событие пришло бы раньше, чем страница начала его слушать. Саму ссылку не пишем
//! ни в какой журнал — токен в ней секрет.

use klick_core::deeplink;
use serde::Serialize;
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager};

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct PendingLink {
    /// Ссылка подписки. Нет — ссылка не прошла проверки: окно скажет «Ссылка не подходит».
    pub url: Option<String>,
    pub name: Option<String>,
    /// Домен подписки: окно показывает его вместо всей ссылки.
    pub host: Option<String>,
}

/// Ссылка, которую страница окна ещё не забрала.
#[derive(Default)]
pub struct Pending(Mutex<Option<PendingLink>>);

/// Что показать по ссылке. `None` — это не `klick://add`: молча ничего не делаем.
fn pending(raw: &str) -> Option<PendingLink> {
    // http://127.0.0.1 и http://localhost — только в отладочной сборке, для панели на своей машине.
    match deeplink::parse(raw, cfg!(debug_assertions)) {
        Ok(None) => None,
        Ok(Some(l)) => Some(PendingLink { host: deeplink::host(&l.url), url: Some(l.url), name: l.name }),
        Err(_) => Some(PendingLink { url: None, name: None, host: None }),
    }
}

/// Ссылка пришла: из аргументов при запуске, от второй копии (Windows) или от системы (macOS).
pub fn open(app: &AppHandle, raw: &str) {
    let Some(link) = pending(raw) else { return };
    if let Some(state) = app.try_state::<Pending>() {
        *state.0.lock().unwrap_or_else(|e| e.into_inner()) = Some(link);
    }
    crate::show_main(app);
    let _ = app.emit_to("main", "klick://add-link", ());
}

/// Отдать ожидающую ссылку и забыть её: второй раз экран «Добавить» по ней не откроется.
#[tauri::command]
pub fn take_pending_link(state: tauri::State<'_, Pending>) -> Option<PendingLink> {
    state.0.lock().unwrap_or_else(|e| e.into_inner()).take()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn window_gets_host_and_never_a_rejected_link() {
        let p = pending("klick://add?url=https%3A%2F%2FSub.Example.com%2Fs%2Ftoken&name=VPN").unwrap();
        assert_eq!(p, PendingLink { url: Some("https://Sub.Example.com/s/token".into()), name: Some("VPN".into()), host: Some("sub.example.com".into()) });
        assert_eq!(pending("klick://add?url=javascript%3Aalert(1)"), Some(PendingLink { url: None, name: None, host: None }));
        assert_eq!(pending("klick://settings"), None);
        assert_eq!(pending("--hidden"), None);
    }
}
