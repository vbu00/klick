//! REST API mihomo на 127.0.0.1: переключение сервера и режима, проверка
//! задержки, перезагрузка конфига, скорость, соединения.

use serde_json::Value;
use std::collections::HashMap;
use std::io::BufRead;
use std::time::Duration;

#[derive(Clone)]
pub struct Api {
    pub port: u16,
    pub secret: String,
}

fn enc(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => out.push(b as char),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

pub const TEST_URL: &str = "https://www.gstatic.com/generate_204";

impl Api {
    fn url(&self, path: &str) -> String {
        format!("http://127.0.0.1:{}{}", self.port, path)
    }

    /// Без системных прокси: в режиме «системный прокси» запрос к самому
    /// mihomo иначе ушёл бы в mihomo же.
    fn agent(timeout: Duration) -> ureq::Agent {
        ureq::AgentBuilder::new().timeout(timeout).try_proxy_from_env(false).build()
    }

    fn auth(&self) -> String {
        format!("Bearer {}", self.secret)
    }

    fn get(&self, path: &str, timeout: Duration) -> Result<Value, String> {
        Self::agent(timeout)
            .get(&self.url(path))
            .set("Authorization", &self.auth())
            .call()
            .map_err(|e| e.to_string())?
            .into_json()
            .map_err(|e| e.to_string())
    }

    pub fn alive(&self) -> bool {
        self.get("/version", Duration::from_millis(700)).is_ok()
    }

    /// Задержка через сервер, мс. Ошибка — не ответил за timeout.
    pub fn delay(&self, proxy: &str, timeout_ms: u32) -> Result<u32, String> {
        let path = format!("/proxies/{}/delay?url={}&timeout={timeout_ms}", enc(proxy), enc(TEST_URL));
        let v = self.get(&path, Duration::from_millis(timeout_ms as u64 + 2000)).map_err(|_| "нет ответа".to_string())?;
        v.get("delay").and_then(Value::as_u64).map(|d| d as u32).ok_or_else(|| "нет ответа".into())
    }

    /// Задержка всех серверов группы разом: имя → мс. Не ответившие
    /// в ответ не попадают.
    pub fn group_delay(&self, group: &str, timeout_ms: u32) -> HashMap<String, u32> {
        let path = format!("/group/{}/delay?url={}&timeout={timeout_ms}", enc(group), enc(TEST_URL));
        let Ok(v) = self.get(&path, Duration::from_millis(timeout_ms as u64 + 4000)) else { return HashMap::new() };
        v.as_object()
            .map(|o| o.iter().filter_map(|(k, v)| v.as_u64().filter(|d| *d > 0).map(|d| (k.clone(), d as u32))).collect())
            .unwrap_or_default()
    }

    pub fn select(&self, group: &str, member: &str) -> Result<(), String> {
        Self::agent(Duration::from_secs(3))
            .put(&self.url(&format!("/proxies/{}", enc(group))))
            .set("Authorization", &self.auth())
            .send_json(serde_json::json!({ "name": member }))
            .map(|_| ())
            .map_err(|e| e.to_string())
    }

    /// rule / global / direct — на лету.
    pub fn set_mode(&self, mode: &str) -> Result<(), String> {
        Self::agent(Duration::from_secs(3))
            .request("PATCH", &self.url("/configs"))
            .set("Authorization", &self.auth())
            .send_json(serde_json::json!({ "mode": mode }))
            .map(|_| ())
            .map_err(|e| e.to_string())
    }

    /// Перечитать конфиг с диска без перезапуска ядра.
    pub fn reload(&self, path: &std::path::Path) -> Result<(), String> {
        Self::agent(Duration::from_secs(15))
            .put(&self.url("/configs?force=true"))
            .set("Authorization", &self.auth())
            .send_json(serde_json::json!({ "path": path.display().to_string() }))
            .map(|_| ())
            .map_err(|e| match e {
                ureq::Error::Status(_, r) => r.into_string().unwrap_or_default(),
                e => e.to_string(),
            })
    }

    /// Сколько скачала каждая программа (по имени exe) за всё время её
    /// соединений — два замера подряд дают скорость.
    pub fn download_by_process(&self) -> HashMap<String, u64> {
        let mut out = HashMap::new();
        let Ok(v) = self.get("/connections", Duration::from_secs(2)) else { return out };
        for c in v.get("connections").and_then(Value::as_array).into_iter().flatten() {
            let name = c.pointer("/metadata/process").and_then(Value::as_str).unwrap_or("");
            if name.is_empty() {
                continue;
            }
            *out.entry(name.to_lowercase()).or_insert(0) += c.get("download").and_then(Value::as_u64).unwrap_or(0);
        }
        out
    }

    /// Поток скорости: раз в секунду (вверх, вниз) байт/с. Блокирует, пока
    /// ядро живо и `keep` велит продолжать.
    pub fn stream_traffic(&self, mut on: impl FnMut(u64, u64), keep: impl Fn() -> bool) {
        let Ok(resp) = ureq::AgentBuilder::new()
            .timeout_connect(Duration::from_secs(2))
            .try_proxy_from_env(false)
            .build()
            .get(&self.url("/traffic"))
            .set("Authorization", &self.auth())
            .call()
        else {
            return;
        };
        for line in std::io::BufReader::new(resp.into_reader()).lines() {
            let Ok(line) = line else { break };
            if !keep() {
                break;
            }
            if let Ok(v) = serde_json::from_str::<Value>(&line) {
                on(v["up"].as_u64().unwrap_or(0), v["down"].as_u64().unwrap_or(0));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn кодирование_имён() {
        assert_eq!(super::enc("Нидерланды · A"), "%D0%9D%D0%B8%D0%B4%D0%B5%D1%80%D0%BB%D0%B0%D0%BD%D0%B4%D1%8B%20%C2%B7%20A");
    }
}
