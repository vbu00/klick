//! Иконки сайтов в правилах и в Kill Switch. Перенесено из Klutz.
//!
//! Качаем сами и только с самого сайта. В макете иконки брались у сервиса
//! Google (s2/favicons) — надёжнее, но тогда весь список сайтов, которые
//! человек пускает мимо VPN или прячет за Kill Switch, уходил бы третьей
//! стороне. Здесь запрос идёт ровно туда же, куда и так ходит пользователь,
//! и по тем же правилам маршрутизации.
//!
//! Окну отдаём готовый `data:`-URI, а не ссылку. Поэтому картинку не грузит
//! WebView (CSP так и остаётся без внешних источников), и попасть в окно
//! может только то, что мы сами проверили и ограничили по размеру.

use std::io::Read;
use std::path::PathBuf;
use std::process::Command;
use std::time::{Duration, SystemTime};

use tauri::{AppHandle, Manager};

#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

/// Главная страница нужна только ради `<link rel="icon">` в шапке.
const MAX_PAGE: u64 = 512 * 1024;
/// Сама иконка. Больше четверти мегабайта favicon не бывает, а вот
/// «картинкой» может прикинуться что угодно длинное.
const MAX_ICON: u64 = 256 * 1024;
/// Сколько ждать. Иконка — украшение, ради неё нельзя ничего подвешивать.
const TIMEOUT: &str = "8";
/// Как долго помнить, что иконки нет. Без этого каждый показ списка заново
/// дёргал бы недоступный сайт; с вечной памятью иконка не появилась бы и
/// после включения VPN (или когда Kill Switch перестал закрывать сайт).
const NEGATIVE_TTL: Duration = Duration::from_secs(6 * 60 * 60);

/// Публичные суффиксы из двух меток, которые реально встречаются. Полный Public Suffix List тянуть ради иконок не
/// стоит: ошибка здесь стоит одной ненайденной картинки.
const TWO_LABEL_TLDS: &[&str] = &[
    "co.uk", "org.uk", "me.uk", "co.jp", "co.kr", "com.br", "com.au", "com.tr", "com.cn",
    "com.mx", "com.ar", "com.pl", "co.nz", "co.za", "com.ua", "com.sg", "com.hk", "com.ru", "org.ru", "net.ru", "msk.ru", "spb.ru", "com.kz",
];

/// Домен бренда: у mail.google.com иконка — на google.com.
pub fn brand_domain(host: &str) -> String {
    let host = host.trim().trim_end_matches('.').to_lowercase();
    let parts: Vec<&str> = host.split('.').filter(|p| !p.is_empty()).collect();
    if parts.len() <= 2 {
        return parts.join(".");
    }
    let last_two = parts[parts.len() - 2..].join(".");
    let keep = if TWO_LABEL_TLDS.contains(&last_two.as_str()) { 3 } else { 2 };
    if parts.len() <= keep {
        return parts.join(".");
    }
    parts[parts.len() - keep..].join(".")
}

/// Имя хоста, а не что-нибудь ещё: значение приходит из поля ввода, а мы
/// подставляем его в URL и в имя файла кэша.
fn valid_host(host: &str) -> bool {
    !host.is_empty()
        && host.len() <= 253
        && host.contains('.')
        && host
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-')
        && !host.starts_with('-')
        && !host.starts_with('.')
}

fn cache_dir(app: &AppHandle) -> PathBuf {
    let dir = app.state::<crate::state::AppState>().dir.join("favicons");
    let _ = std::fs::create_dir_all(&dir);
    dir
}

/// Кэш — это готовый `data:`-URI текстом. Так не нужно отдельно помнить тип
/// картинки: он уже внутри строки. Пустой файл значит «иконки нет».
fn cache_path(app: &AppHandle, brand: &str) -> PathBuf {
    cache_dir(app).join(format!("{brand}.txt"))
}

fn curl(url: &str, max_bytes: u64) -> Result<Vec<u8>, String> {
    #[allow(unused_mut)]
    let mut cmd = Command::new(crate::sys::system_exe("curl.exe"));
    cmd.args([
        "-fsSL",
        // Только https и только по https редиректы: ответ мы показываем
        // пользователю картинкой, и подменить его по дороге не должны.
        "--proto",
        "=https",
        "--proto-redir",
        "=https",
        "--max-redirs",
        "5",
        "-m",
        TIMEOUT,
        // Обрывает закачку, если сервер объявил размер больше нашего. От
        // сервера, который соврал в Content-Length, защищает обрезка ниже.
        "--max-filesize",
        &max_bytes.to_string(),
        "-H",
        "User-Agent: klick",
        url,
    ]);
    #[cfg(target_os = "windows")]
    cmd.creation_flags(crate::sys::CREATE_NO_WINDOW);
    cmd.stdout(std::process::Stdio::piped());
    cmd.stderr(std::process::Stdio::null());

    let mut child = cmd.spawn().map_err(|e| e.to_string())?;
    let mut buf = Vec::new();
    if let Some(mut out) = child.stdout.take() {
        // Читаем не больше лимита независимо от того, что обещал сервер.
        let _ = out.by_ref().take(max_bytes + 1).read_to_end(&mut buf);
    }
    let status = child.wait().map_err(|e| e.to_string())?;
    if !status.success() {
        return Err("не ответил".into());
    }
    if buf.len() as u64 > max_bytes {
        return Err("слишком большой ответ".into());
    }
    Ok(buf)
}

/// Тип картинки по сигнатуре, а не по расширению или заголовку: и то и
/// другое задаёт чужой сервер.
fn image_mime(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(&[0x89, b'P', b'N', b'G']) {
        return Some("image/png");
    }
    if bytes.starts_with(&[0x00, 0x00, 0x01, 0x00]) {
        return Some("image/x-icon");
    }
    if bytes.starts_with(b"GIF8") {
        return Some("image/gif");
    }
    if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        return Some("image/jpeg");
    }
    if bytes.len() > 12 && bytes.starts_with(b"RIFF") && &bytes[8..12] == b"WEBP" {
        return Some("image/webp");
    }
    // SVG показываем через <img>, а там скрипты внутри картинки не
    // выполняются — в отличие от <object> или вставки в разметку.
    let head = &bytes[..bytes.len().min(512)];
    let text = String::from_utf8_lossy(head);
    let t = text.trim_start();
    if t.starts_with("<svg") || (t.starts_with("<?xml") && text.contains("<svg")) {
        return Some("image/svg+xml");
    }
    None
}

const B64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

fn base64(data: &[u8]) -> String {
    let mut s = String::with_capacity(data.len().div_ceil(3) * 4);
    for c in data.chunks(3) {
        let b = [c[0], *c.get(1).unwrap_or(&0), *c.get(2).unwrap_or(&0)];
        let n = ((b[0] as u32) << 16) | ((b[1] as u32) << 8) | b[2] as u32;
        s.push(B64[(n >> 18) as usize & 63] as char);
        s.push(B64[(n >> 12) as usize & 63] as char);
        s.push(if c.len() > 1 { B64[(n >> 6) as usize & 63] as char } else { '=' });
        s.push(if c.len() > 2 { B64[n as usize & 63] as char } else { '=' });
    }
    s
}

/// Ссылки на иконку из шапки страницы, в порядке предпочтения.
///
/// `sizes` у `<link>` необязателен и часто врёт, но когда он есть — это
/// единственная подсказка, какая из иконок крупнее.
fn icon_hrefs(html: &str) -> Vec<String> {
    // Режем по границе символа: байтовый срез посреди многобайтового
    // символа — паника, а с panic = "abort" это падение всего приложения
    // посреди работы. Страницы больше 200 КБ с не-ASCII текстом — норма.
    let mut cut = html.len().min(200 * 1024);
    while !html.is_char_boundary(cut) {
        cut -= 1;
    }
    let head = &html[..cut];
    // Только ASCII: полный to_lowercase() меняет длину строки в байтах
    // (İ → i̇, знак Кельвина → k), и смещения, найденные в `lower`,
    // указывали бы в `head` не туда — вплоть до среза не по границе символа.
    let lower = head.to_ascii_lowercase();
    let mut found: Vec<(u32, String)> = Vec::new();

    let mut pos = 0usize;
    while let Some(rel) = lower[pos..].find("<link") {
        let start = pos + rel;
        let end = match lower[start..].find('>') {
            Some(e) => start + e,
            None => break,
        };
        let tag = &head[start..end];
        let tag_lower = &lower[start..end];
        pos = end + 1;

        let rel_val = attr(tag_lower, "rel").unwrap_or_default();
        if !rel_val.contains("icon") {
            continue;
        }
        let Some(href) = attr(tag, "href") else { continue };
        if href.trim().is_empty() {
            continue;
        }
        // Явный размер всегда главнее догадки: у Riot, например, четыре
        // apple-touch-icon от 76 до 180, и раньше все они получали одинаковый
        // вес — порядок между ними выходил случайным.
        let mut weight = attr(tag_lower, "sizes")
            .and_then(|s| s.split(['x', 'X']).next().and_then(|n| n.trim().parse::<u32>().ok()))
            .unwrap_or(0);
        if weight == 0 && rel_val.contains("apple-touch") {
            // Без размера в разметке apple-touch-icon почти всегда 180×180 —
            // всё равно крупнее безымянного favicon.
            weight = 180;
        }
        found.push((weight, href.trim().to_string()));
    }
    // По убыванию размера; сортировка устойчива, поэтому среди равных
    // сохраняется порядок из разметки.
    found.sort_by_key(|(weight, _)| std::cmp::Reverse(*weight));
    found.into_iter().map(|(_, h)| h).collect()
}

fn attr(tag: &str, name: &str) -> Option<String> {
    let at = tag.find(&format!("{name}="))?;
    let rest = &tag[at + name.len() + 1..];
    let mut chars = rest.chars();
    let quote = chars.next()?;
    if quote == '"' || quote == '\'' {
        let end = rest[1..].find(quote)?;
        Some(rest[1..1 + end].to_string())
    } else {
        let end = rest.find([' ', '\t', '\n', '>']).unwrap_or(rest.len());
        Some(rest[..end].to_string())
    }
}

/// Приводим href из разметки к абсолютному https-адресу. Всё, что не
/// сводится к https, отбрасываем — качать иконку по http означало бы
/// показать пользователю картинку, которую мог подменить кто угодно.
fn absolutize(href: &str, brand: &str) -> Option<String> {
    let h = href.trim();
    if h.starts_with("https://") {
        return Some(h.to_string());
    }
    if h.starts_with("//") {
        return Some(format!("https:{h}"));
    }
    if h.starts_with("http://") || h.contains("://") || h.starts_with("data:") {
        return None;
    }
    if let Some(rest) = h.strip_prefix('/') {
        return Some(format!("https://{brand}/{rest}"));
    }
    Some(format!("https://{brand}/{h}"))
}

/// Иконка сервиса как `data:`-URI. `Ok(None)` — сходили и не нашли.
pub fn get(app: &AppHandle, host: &str) -> Result<Option<String>, String> {
    if !valid_host(host) {
        return Err("это не имя хоста".into());
    }
    let brand = brand_domain(host);
    if brand.is_empty() {
        return Err("не удалось определить домен".into());
    }
    let path = cache_path(app, &brand);

    if let Ok(meta) = std::fs::metadata(&path) {
        let fresh = meta
            .modified()
            .ok()
            .and_then(|m| SystemTime::now().duration_since(m).ok())
            .map(|age| age < NEGATIVE_TTL)
            .unwrap_or(false);
        if meta.len() > 0 {
            if let Ok(s) = std::fs::read_to_string(&path) {
                return Ok(Some(s));
            }
        } else if fresh {
            // Пустой файл — «в прошлый раз не нашли», и с тех пор прошло
            // меньше суток.
            return Ok(None);
        }
    }

    let found = lookup(&brand);
    match &found {
        Some(uri) => {
            let _ = std::fs::write(&path, uri);
        }
        None => {
            let _ = std::fs::write(&path, b"");
        }
    }
    Ok(found)
}

fn lookup(brand: &str) -> Option<String> {
    let mut candidates: Vec<String> = Vec::new();
    if let Ok(page) = curl(&format!("https://{brand}/"), MAX_PAGE) {
        let html = String::from_utf8_lossy(&page);
        for href in icon_hrefs(&html) {
            if let Some(abs) = absolutize(&href, brand) {
                candidates.push(abs);
            }
        }
    }
    // Даже если страница не открылась или ссылок в ней нет — /favicon.ico
    // лежит по соглашению, и очень часто он там есть.
    candidates.push(format!("https://{brand}/favicon.ico"));

    for url in candidates.into_iter().take(4) {
        if let Ok(bytes) = curl(&url, MAX_ICON) {
            if let Some(mime) = image_mime(&bytes) {
                return Some(format!("data:{mime};base64,{}", base64(&bytes)));
            }
        }
    }
    None
}

#[cfg(test)]
mod unit_tests {
    use super::*;

    #[test]
    fn домен_бренда_отбрасывает_служебные_поддомены() {
        assert_eq!(brand_domain("api.rlpp.psynet.gg"), "psynet.gg");
        assert_eq!(brand_domain("title.mgt.xboxlive.com"), "xboxlive.com");
        assert_eq!(brand_domain("steampowered.com"), "steampowered.com");
        assert_eq!(brand_domain("WWW.YouTube.com"), "youtube.com");
        assert_eq!(brand_domain("one.one.one.one"), "one.one");
    }

    #[test]
    fn двухсоставные_домены_не_срезаются_до_мусора() {
        // Срежь здесь до двух меток — получится «co.uk», и иконку мы пошли
        // бы искать не у того.
        assert_eq!(brand_domain("api.service.co.uk"), "service.co.uk");
        assert_eq!(brand_domain("shop.com.br"), "shop.com.br");
    }

    #[test]
    fn имя_хоста_проверяется_до_подстановки_в_url() {
        assert!(valid_host("api.steampowered.com"));
        assert!(!valid_host("localhost"));
        assert!(!valid_host("evil.com/../../etc"));
        assert!(!valid_host("evil.com?x=1"));
        assert!(!valid_host(""));
    }

    #[test]
    fn тип_картинки_по_сигнатуре() {
        assert_eq!(image_mime(&[0x89, b'P', b'N', b'G', 0, 0]), Some("image/png"));
        assert_eq!(image_mime(&[0, 0, 1, 0, 1, 0]), Some("image/x-icon"));
        assert_eq!(image_mime(b"<svg xmlns='...'></svg>"), Some("image/svg+xml"));
        // HTML-страница с ошибкой вместо картинки — не картинка.
        assert_eq!(image_mime(b"<!DOCTYPE html><html>404"), None);
        assert_eq!(image_mime(b""), None);
    }

    #[test]
    fn base64_как_в_учебнике() {
        assert_eq!(base64(b""), "");
        assert_eq!(base64(b"f"), "Zg==");
        assert_eq!(base64(b"fo"), "Zm8=");
        assert_eq!(base64(b"foo"), "Zm9v");
        assert_eq!(base64(b"foobar"), "Zm9vYmFy");
        assert_eq!(base64(&[0xFF, 0xFE, 0xFD]), "//79");
    }

    #[test]
    fn ссылки_на_иконки_по_убыванию_размера() {
        let html = r#"<head>
          <link rel="icon" href="/small.png" sizes="16x16">
          <link rel="apple-touch-icon" href="/touch.png">
          <link rel="shortcut icon" href="/big.png" sizes="192x192">
          <link rel="stylesheet" href="/style.css">
        </head>"#;
        assert_eq!(icon_hrefs(html), vec!["/big.png", "/touch.png", "/small.png"]);
    }

    #[test]
    fn среди_apple_touch_побеждает_тот_что_крупнее() {
        // Разметка как на riotgames.com: несколько apple-touch-icon с явными
        // размерами плюс безымянный shortcut icon.
        let html = r#"<head>
          <link rel="apple-touch-icon-precomposed" sizes="76x76" href="/a76.png"/>
          <link rel="apple-touch-icon-precomposed" sizes="180x180" href="/a180.png"/>
          <link rel="apple-touch-icon" href="/plain.png"/>
          <link rel="shortcut icon" href="/favicon.ico"/>
        </head>"#;
        let got = icon_hrefs(html);
        assert_eq!(got[0], "/a180.png", "самый крупный — первым");
        assert_eq!(got.last().unwrap(), "/favicon.ico", "безымянный — последним");
        assert!(
            got.iter().position(|h| h == "/a76.png").unwrap()
                > got.iter().position(|h| h == "/plain.png").unwrap(),
            "apple-touch без размера считаем за 180, он крупнее явных 76"
        );
    }

    #[test]
    fn относительные_ссылки_разворачиваются_а_http_отбрасывается() {
        assert_eq!(absolutize("/a.png", "x.com").as_deref(), Some("https://x.com/a.png"));
        assert_eq!(absolutize("a.png", "x.com").as_deref(), Some("https://x.com/a.png"));
        assert_eq!(absolutize("//cdn.x.com/a.png", "x.com").as_deref(), Some("https://cdn.x.com/a.png"));
        assert_eq!(absolutize("https://cdn.x.com/a.png", "x.com").as_deref(), Some("https://cdn.x.com/a.png"));
        assert_eq!(absolutize("http://x.com/a.png", "x.com"), None);
        assert_eq!(absolutize("data:image/png;base64,AAAA", "x.com"), None);
    }

    #[test]
    fn большая_страница_с_кириллицей_не_роняет_разбор() {
        // Раньше шапка резалась ровно на 200 КБ по байтам — попади граница
        // внутрь «ё», и это была паника (а значит, падение приложения).
        let mut html = String::from(r#"<link rel="icon" href="/i.png">"#);
        while html.len() < 200 * 1024 + 3 {
            html.push('ё');
        }
        assert_eq!(icon_hrefs(&html), vec!["/i.png"]);
    }

    #[test]
    fn символы_меняющие_длину_в_нижнем_регистре_не_сдвигают_срезы() {
        // to_lowercase() превращает «İ» в два символа, и смещения из
        // строчной копии переставали совпадать с оригиналом.
        let html = r#"<title>İİİİİİ</title><link rel="icon" href="/a.png"><link rel="ICON" href="/b.png">"#;
        assert_eq!(icon_hrefs(html), vec!["/a.png", "/b.png"]);
    }
}
