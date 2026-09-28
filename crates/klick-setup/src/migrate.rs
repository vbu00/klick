//! Перенос подключений из прежней kl!ck (0.2–0.4): её `profiles.dat` зашифрован DPAPI того,
//! кто вошёл в Windows. Установщик работает от его имени (с правами администратора), поэтому
//! может его прочитать — до того, как прежняя kl!ck будет удалена со всеми данными. Новой службе
//! подключения передаются её же каналом, как если бы человек вставил ссылки сам.

use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::time::{Duration, Instant};

/// Одно подключение прежней kl!ck.
#[derive(Clone, Debug, PartialEq)]
pub enum Item {
    /// Ссылка на подписку или одиночная ссылка (`vless://…`).
    Source { name: String, source: String },
    /// Импортированный файл: серверы в формате mihomo.
    File { name: String, content: String },
}

impl Item {
    pub fn name(&self) -> &str {
        match self {
            Item::Source { name, .. } | Item::File { name, .. } => name,
        }
    }
}

/// Подключения из `profiles.dat` в папке данных прежней kl!ck. Пусто — нечего переносить
/// (файла нет, не расшифровался или внутри ничего).
pub fn read_old(data_dir: &Path) -> Vec<Item> {
    let Ok(enc) = std::fs::read(data_dir.join("profiles.dat")) else { return vec![] };
    let Some(plain) = unprotect(&enc) else { return vec![] };
    parse_vault(&plain)
}

/// Разбор расшифрованного `profiles.dat`: `{"profiles":[{kind, name, url, proxies}]}`.
pub fn parse_vault(plain: &[u8]) -> Vec<Item> {
    let Ok(v) = serde_json::from_slice::<Value>(plain) else { return vec![] };
    let mut out = vec![];
    for p in v.get("profiles").and_then(Value::as_array).into_iter().flatten() {
        let name = p.get("name").and_then(Value::as_str).unwrap_or("").trim().to_string();
        let url = p.get("url").and_then(Value::as_str).map(str::trim).filter(|u| !u.is_empty());
        match (p.get("kind").and_then(Value::as_str), url) {
            (Some("sub" | "single"), Some(u)) => out.push(Item::Source { name, source: u.to_string() }),
            _ => {
                let proxies: Vec<&Value> = p.get("proxies").and_then(Value::as_array).map(|a| a.iter().collect()).unwrap_or_default();
                if proxies.is_empty() {
                    continue;
                }
                // JSON-объект — это и YAML: служба узнаёт файл mihomo по строке `proxies:`.
                let mut content = String::from("proxies:\n");
                for px in proxies {
                    content.push_str("  - ");
                    content.push_str(&px.to_string());
                    content.push('\n');
                }
                out.push(Item::File { name, content });
            }
        }
    }
    out
}

/// Передать подключения службе. Служба только что запущена — канал ждём до 30 секунд.
/// Возвращает, сколько перенеслось, и имена тех, что не вышло.
pub fn push(pipe: &str, items: &[Item]) -> (usize, Vec<String>) {
    if items.is_empty() {
        return (0, vec![]);
    }
    let started = Instant::now();
    let file = loop {
        match std::fs::OpenOptions::new().read(true).write(true).open(pipe) {
            Ok(f) => break Some(f),
            Err(_) if started.elapsed() < Duration::from_secs(30) => std::thread::sleep(Duration::from_millis(500)),
            Err(_) => break None,
        }
    };
    let Some(file) = file else { return (0, items.iter().map(|i| i.name().to_string()).collect()) };
    let Ok(mut writer) = file.try_clone() else { return (0, items.iter().map(|i| i.name().to_string()).collect()) };
    let mut reader = BufReader::new(file);
    let (mut ok, mut failed) = (0, vec![]);
    for (i, item) in items.iter().enumerate() {
        let id = 1000 + i as u64;
        let req = match item {
            Item::Source { name, source } => json!({ "id": id, "cmd": "add_connection", "args": { "source": source, "name": if name.is_empty() { Value::Null } else { json!(name) } } }),
            Item::File { name, content } => json!({ "id": id, "cmd": "import_file", "args": { "file_name": format!("{}.yaml", if name.is_empty() { "kl!ck" } else { name }), "content": content } }),
        };
        let sent = writer.write_all(format!("{req}\n").as_bytes()).is_ok();
        let answered = sent && wait_answer(&mut reader, id);
        if answered {
            ok += 1;
        } else {
            failed.push(item.name().to_string());
        }
    }
    (ok, failed)
}

/// Ответ службы на запрос `id`: `true`, если `ok`. События по пути пропускаем.
fn wait_answer(reader: &mut impl BufRead, id: u64) -> bool {
    let mut line = String::new();
    loop {
        line.clear();
        match reader.read_line(&mut line) {
            Ok(0) | Err(_) => return false,
            Ok(_) => {
                let Ok(v) = serde_json::from_str::<Value>(&line) else { continue };
                if v["t"] == "res" && v["id"] == json!(id) {
                    return v["ok"] == json!(true);
                }
            }
        }
    }
}

fn unprotect(data: &[u8]) -> Option<Vec<u8>> {
    use windows::Win32::Foundation::{LocalFree, HLOCAL};
    use windows::Win32::Security::Cryptography::{CryptUnprotectData, CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB};
    let input = CRYPT_INTEGER_BLOB { cbData: data.len() as u32, pbData: data.as_ptr() as *mut u8 };
    let mut out = CRYPT_INTEGER_BLOB::default();
    unsafe {
        CryptUnprotectData(&input, None, None, None, None, CRYPTPROTECT_UI_FORBIDDEN, &mut out).ok()?;
        let bytes = std::slice::from_raw_parts(out.pbData, out.cbData as usize).to_vec();
        let _ = LocalFree(HLOCAL(out.pbData as _));
        Some(bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn old_profiles_become_links_and_files() {
        let vault = r#"{"profiles":[
            {"id":"a","kind":"sub","name":"Remnawave","url":"https://panel.example/sub/xyz","proxies":[{"name":"NL"}]},
            {"id":"b","kind":"single","name":"grpc","url":"vless://u@1.2.3.4:443#grpc","proxies":[{"name":"grpc"}]},
            {"id":"c","kind":"file","name":"Мой конфиг","proxies":[{"name":"DE","type":"ss","server":"5.6.7.8","port":443}]},
            {"id":"d","kind":"file","name":"Пустой","proxies":[]}
        ]}"#;
        let items = parse_vault(vault.as_bytes());
        assert_eq!(items.len(), 3);
        assert_eq!(items[0], Item::Source { name: "Remnawave".into(), source: "https://panel.example/sub/xyz".into() });
        assert_eq!(items[1], Item::Source { name: "grpc".into(), source: "vless://u@1.2.3.4:443#grpc".into() });
        let Item::File { content, .. } = &items[2] else { panic!() };
        assert!(content.starts_with("proxies:\n  - {"));
        assert_eq!(klick_core::sub::inspect_content(content), klick_core::sub::Content::ClashYaml);
    }
}
