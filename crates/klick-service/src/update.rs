//! Обновление в один клик. Служба смотрит последний выпуск на GitHub, по кнопке в окне скачивает
//! установщик, проверяет подпись владельца и ставит его без окна — без вопроса «Разрешить
//! изменения?» или пароля администратора: служба и так работает от имени системы (root на Mac).
//! Windows — `klick-setup.exe --silent`, macOS — `installer -pkg klick-<версия>.pkg`.
//!
//! Ставится только подписанное: Ed25519 владельца над `klick-update-v1\n<имя файла>\n<sha256>\n`.
//! Закрытый ключ — только у владельца (`tools/sign-release.py`), не в репозитории и не в CI: кто
//! может выпускать релизы, тот ещё не может выпустить обновление, которое поставится само. Имя
//! файла с версией не даёт выдать старый подписанный установщик за новый. Без подписи окно
//! предлагает скачать выпуск вручную, как раньше.
//!
//! Установщик останавливает службу и окно. Перед запуском служба оставляет отметку
//! `update.json`; новая служба (или прежняя, если установщик откатился) по ней понимает, чем
//! кончилось, и говорит об этом окну. Окно снова открывает на Windows служба, на Mac — сам пакет
//! (postinstall). VPN, если был включён, служба восстанавливает сама: остановка не стирает
//! отметку «был включён» (`runtime.json`).

use crate::paths::Paths;
use klick_proto::{ErrorInfo, UpdateView};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Репозиторий с выпусками (`владелец/имя`) задаётся при сборке для публикации: `KLICK_RELEASES_REPO=vbu00/klick`.
/// До публикации проверка обновлений выключена (архитектура, следствия А2.1).
const RELEASES_REPO: Option<&str> = option_env!("KLICK_RELEASES_REPO");

/// Открытый ключ подписи выпусков vbu00/klick. Своя сборка со своими выпусками — свой ключ: `KLICK_UPDATE_KEY`.
const UPDATE_KEY: &str = match option_env!("KLICK_UPDATE_KEY") {
    Some(k) => k,
    None => "1iGUgEvJnEpkjhopB/Dbs/MGc1up9VgsIcyU3Cv0bRY=",
};

/// Установщик больше этого — не наш (сейчас ~40 МБ).
const MAX_INSTALLER: u64 = 300 << 20;
/// Где умеем ставить сами.
const SUPPORTED: bool = cfg!(any(windows, target_os = "macos"));
/// Отметка старше часа — не про это обновление (компьютер выключили посреди установки и т. п.).
const PENDING_FRESH_SECS: i64 = 3600;

/// Установщик этой системы в выпуске.
pub fn installer_name(version: &str) -> String {
    if cfg!(target_os = "macos") {
        format!("klick-{version}.pkg")
    } else {
        format!("klick-{version}-x64-setup.exe")
    }
}

/// Что подписывает владелец.
pub fn signed_message(name: &str, sha256_hex: &str) -> String {
    format!("klick-update-v1\n{name}\n{sha256_hex}\n")
}

/// Подпись `.sig` (base64 от 64 байт) сделана ключом `key_b64` над установщиком `name` с этим sha256.
pub fn signature_ok(key_b64: &str, name: &str, sha256_hex: &str, sig_text: &str) -> bool {
    use base64::Engine as _;
    let b64 = base64::engine::general_purpose::STANDARD;
    let (Ok(key), Ok(sig)) = (b64.decode(key_b64.trim()), b64.decode(sig_text.trim())) else {
        return false;
    };
    if key.len() != 32 || sig.len() != 64 {
        return false;
    }
    ring::signature::UnparsedPublicKey::new(&ring::signature::ED25519, &key).verify(signed_message(name, sha256_hex).as_bytes(), &sig).is_ok()
}

/// `0.4.10` новее `0.4.9`: сравниваем числа, а не строки.
pub fn version_newer(latest: &str, current: &str) -> bool {
    let parse = |v: &str| v.split(['.', '-', '+']).take(3).map(|x| x.parse::<u32>().unwrap_or(0)).collect::<Vec<_>>();
    parse(latest) > parse(current)
}

fn current() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

fn client(timeout: Duration) -> Result<reqwest::Client, ErrorInfo> {
    reqwest::Client::builder()
        .timeout(timeout)
        .connect_timeout(Duration::from_secs(10))
        .user_agent(concat!("klick/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|_| ErrorInfo::new("update.unreachable"))
}

#[derive(Clone, Debug)]
struct Asset {
    url: String,
    size: u64,
}

#[derive(Clone, Debug)]
struct Release {
    version: String,
    page: Option<String>,
    installer: Option<Asset>,
    signature: Option<Asset>,
}

impl Release {
    /// Ставится в один клик: Windows или Mac, есть установщик этой версии и его подпись.
    fn installable(&self) -> bool {
        SUPPORTED && self.installer.is_some() && self.signature.is_some()
    }
}

/// Последний выпуск на GitHub; `None` — выпусков нет.
async fn latest() -> Result<Option<Release>, ErrorInfo> {
    let Some(repo) = RELEASES_REPO.filter(|r| !r.is_empty()) else {
        return Err(ErrorInfo::new("update.disabled"));
    };
    let url = format!("https://api.github.com/repos/{repo}/releases/latest");
    let resp = client(Duration::from_secs(10))?
        .get(&url)
        .header("Accept", "application/vnd.github+json")
        .send()
        .await
        .map_err(|_| ErrorInfo::new("update.unreachable"))?;
    if resp.status() == reqwest::StatusCode::NOT_FOUND {
        return Ok(None);
    }
    if !resp.status().is_success() {
        return Err(ErrorInfo::with("update.http_status", json!({ "status": resp.status().as_u16() })));
    }
    let v: Value = resp.json().await.map_err(|_| ErrorInfo::new("update.bad_answer"))?;
    Ok(Some(parse_release(&v)))
}

fn parse_release(v: &Value) -> Release {
    let version = v["tag_name"].as_str().unwrap_or_default().trim_start_matches('v').to_string();
    let name = installer_name(&version);
    let asset = |want: &str| {
        v["assets"].as_array()?.iter().find(|a| a["name"].as_str() == Some(want)).and_then(|a| {
            Some(Asset { url: a["browser_download_url"].as_str()?.to_string(), size: a["size"].as_u64().unwrap_or(0) })
        })
    };
    Release {
        installer: asset(&name),
        signature: asset(&format!("{name}.sig")),
        page: v["html_url"].as_str().map(str::to_string),
        version,
    }
}

/// Есть ли новая версия и можно ли поставить её в один клик.
pub async fn check() -> Result<UpdateView, ErrorInfo> {
    let current = current();
    let Some(r) = latest().await? else {
        return Ok(UpdateView { current, latest: None, url: None, newer: false, installable: false });
    };
    let newer = version_newer(&r.version, &current);
    let installable = newer && r.installable();
    Ok(UpdateView { current, latest: (!r.version.is_empty()).then(|| r.version.clone()), url: r.page.clone(), newer, installable })
}

// ── Установка ────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize)]
struct Pending {
    from: String,
    to: String,
    at: i64,
}

fn pending_path(paths: &Paths) -> PathBuf {
    paths.data.join("update.json")
}

fn updates_dir(paths: &Paths) -> PathBuf {
    paths.data.join("updates")
}

/// Как идёт обновление — окну: этап (`download`, `verify`, `install`), скачано и всего байт.
pub type Progress<'a> = &'a (dyn Fn(&str, u64, u64) + Send + Sync);

/// Скачать, проверить и запустить установщик последнего выпуска. `Ok(версия)` — установщик
/// запущен: скоро он остановит службу и окно, а потом новая служба откроет окно снова.
pub async fn install(paths: &Paths, progress: Progress<'_>) -> Result<String, ErrorInfo> {
    if !SUPPORTED {
        return Err(ErrorInfo::new("update.manual"));
    }
    let release = latest().await?.ok_or_else(|| ErrorInfo::new("update.none"))?;
    if !version_newer(&release.version, &current()) {
        return Err(ErrorInfo::new("update.none"));
    }
    let name = installer_name(&release.version);
    let (Some(installer), Some(signature)) = (&release.installer, &release.signature) else {
        return Err(ErrorInfo::new("update.unsigned"));
    };
    if installer.size == 0 || installer.size > MAX_INSTALLER || signature.size > 1024 {
        return Err(ErrorInfo::new("update.bad_answer"));
    }

    let http = client(Duration::from_secs(600))?;
    let sig = http
        .get(&signature.url)
        .send()
        .await
        .and_then(|r| r.error_for_status())
        .map_err(|_| ErrorInfo::new("update.download"))?
        .text()
        .await
        .map_err(|_| ErrorInfo::new("update.download"))?;

    // Папка данных службы — только для администраторов и системы: проверенный файл никто не подменит.
    let dir = updates_dir(paths);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).map_err(|_| ErrorInfo::new("update.download"))?;
    let part = dir.join(format!("{name}.part"));
    let digest = download(&http, &installer.url, &part, installer.size, progress).await.inspect_err(|_| {
        let _ = std::fs::remove_file(&part);
    })?;
    progress("verify", installer.size, installer.size);
    if !signature_ok(UPDATE_KEY, &name, &digest, &sig) {
        let _ = std::fs::remove_file(&part);
        tracing::warn!("обновление {}: подпись не прошла проверку, установка отменена", release.version);
        return Err(ErrorInfo::new("update.bad_signature"));
    }
    let exe = dir.join(&name);
    std::fs::rename(&part, &exe).map_err(|_| ErrorInfo::new("update.download"))?;

    let pending = Pending { from: current(), to: release.version.clone(), at: crate::engine::unix_now() };
    crate::storage::write_atomic(&pending_path(paths), &serde_json::to_vec(&pending).map_err(|_| ErrorInfo::new("update.start_failed"))?)
        .map_err(|_| ErrorInfo::new("update.start_failed"))?;
    progress("install", installer.size, installer.size);
    if let Err(e) = start_installer(&exe, &paths.logs.join("update.log")) {
        let _ = std::fs::remove_file(pending_path(paths));
        tracing::error!("установщик обновления не запустился: {e}");
        return Err(ErrorInfo::new("update.start_failed"));
    }
    tracing::info!("обновление {} → {}: подпись верна, установщик запущен", pending.from, pending.to);
    Ok(release.version)
}

/// Скачать в `path` ровно `size` байт; sha256 скачанного.
async fn download(http: &reqwest::Client, url: &str, path: &Path, size: u64, progress: Progress<'_>) -> Result<String, ErrorInfo> {
    use sha2::{Digest, Sha256};
    use std::io::Write;
    let fail = |_| ErrorInfo::new("update.download");
    let mut resp = http.get(url).send().await.and_then(|r| r.error_for_status()).map_err(fail)?;
    let mut file = std::fs::File::create(path).map_err(|_| ErrorInfo::new("update.download"))?;
    let mut hash = Sha256::new();
    let mut got = 0u64;
    let mut told = std::time::Instant::now();
    progress("download", 0, size);
    while let Some(chunk) = resp.chunk().await.map_err(fail)? {
        got += chunk.len() as u64;
        // Окну — не чаще раза в 300 мс: полоса и так плавная, а событий не тысячи.
        if told.elapsed() >= std::time::Duration::from_millis(300) || got >= size {
            told = std::time::Instant::now();
            progress("download", got.min(size), size);
        }
        if got > size {
            return Err(ErrorInfo::new("update.download"));
        }
        hash.update(&chunk);
        file.write_all(&chunk).map_err(|_| ErrorInfo::new("update.download"))?;
    }
    file.sync_all().map_err(|_| ErrorInfo::new("update.download"))?;
    if got != size {
        return Err(ErrorInfo::new("update.download"));
    }
    Ok(hash.finalize().iter().map(|b| format!("{b:02x}")).collect())
}

/// Установщик — отдельным процессом, не дочерним по смыслу: он остановит эту службу и должен жить дальше.
#[cfg(windows)]
fn start_installer(exe: &Path, log: &Path) -> std::io::Result<()> {
    use std::os::windows::process::CommandExt;
    const DETACHED_PROCESS: u32 = 0x0000_0008;
    const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let out = std::fs::File::create(log)?;
    let err = out.try_clone()?;
    std::process::Command::new(exe)
        .args(["--silent", "--no-migrate"])
        .stdin(std::process::Stdio::null())
        .stdout(out)
        .stderr(err)
        .creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP | CREATE_NO_WINDOW)
        .spawn()
        .map(drop)
}

/// macOS: системный установщик пакетов. Своя группа процессов: launchd, перезапуская службу
/// (postinstall → `klick-service install`), завершает всю её группу — установщик должен пережить это.
#[cfg(target_os = "macos")]
fn start_installer(pkg: &Path, log: &Path) -> std::io::Result<()> {
    use std::os::unix::process::CommandExt;
    let out = std::fs::File::create(log)?;
    let err = out.try_clone()?;
    std::process::Command::new("/usr/sbin/installer")
        .arg("-pkg")
        .arg(pkg)
        .args(["-target", "/"])
        .stdin(std::process::Stdio::null())
        .stdout(out)
        .stderr(err)
        .process_group(0)
        .spawn()
        .map(drop)
}

#[cfg(not(any(windows, target_os = "macos")))]
fn start_installer(_exe: &Path, _log: &Path) -> std::io::Result<()> {
    Err(std::io::Error::other("только Windows и macOS"))
}

/// Служба запустилась: если перед этим она ставила обновление — чем кончилось. Уведомление окну
/// (`update.done` или `update.failed`) и нужно ли открыть окно снова (установщик его закрыл).
pub fn take_result(paths: &Paths) -> Option<(&'static str, Value)> {
    let path = pending_path(paths);
    let pending: Option<Pending> = std::fs::read(&path).ok().and_then(|b| serde_json::from_slice(&b).ok());
    let _ = std::fs::remove_file(&path);
    let _ = std::fs::remove_dir_all(updates_dir(paths));
    let p = pending.filter(|p| (crate::engine::unix_now() - p.at).abs() < PENDING_FRESH_SECS)?;
    if current() == p.to {
        tracing::info!("обновление установлено: {} → {}", p.from, p.to);
        Some(("update.done", json!({ "version": p.to })))
    } else {
        tracing::warn!("обновление до {} не установилось, работает {} (подробности — logs\\update.log)", p.to, current());
        Some(("update.failed", json!({ "version": p.to })))
    }
}

/// Открыть окно kl!ck у того, кто сейчас за компьютером: установщик его закрыл. Служба — от имени
/// системы, поэтому запускаем с токеном пользователя активного сеанса.
#[cfg(windows)]
pub fn reopen_window() {
    let Some(exe) = std::env::current_exe().ok().and_then(|p| p.parent().map(|d| d.join("klick.exe"))).filter(|p| p.is_file()) else {
        return;
    };
    if let Err(e) = crate::userproxy::launch_in_active_session(&exe) {
        tracing::warn!("окно после обновления не открылось: {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Ключ и подпись из tools/sign-release.py (cryptography) — служба проверяет то, что делает этот инструмент.
    const TEST_KEY: &str = "vlSaN9zTTlzc12w8sVbuqQ65H3A5qCz0no3oc4rXWLw=";

    #[test]
    fn versions_compare_as_numbers() {
        assert!(version_newer("0.4.10", "0.4.9"));
        assert!(version_newer("1.0.0", "0.4.0"));
        assert!(!version_newer("0.3.0", "0.4.0"));
        assert!(!version_newer("0.4.0", "0.4.0"));
        assert!(version_newer("0.4.1-beta", "0.4.0"));
    }

    #[test]
    fn release_assets_are_found_by_exact_name() {
        let v = json!({
            "tag_name": "v0.5.3",
            "html_url": "https://github.com/vbu00/klick/releases/tag/v0.5.3",
            "assets": [
                { "name": "klick-macos.pkg", "browser_download_url": "https://x/latest-pkg", "size": 1 },
                { "name": "klick-0.5.3.pkg", "browser_download_url": "https://x/pkg", "size": 56 },
                { "name": "klick-0.5.3.pkg.sig", "browser_download_url": "https://x/pkg-sig", "size": 89 },
                { "name": "klick-0.5.3-x64-setup.exe", "browser_download_url": "https://x/exe", "size": 40 },
                { "name": "klick-0.5.3-x64-setup.exe.sig", "browser_download_url": "https://x/sig", "size": 89 }
            ]
        });
        let r = parse_release(&v);
        assert_eq!(r.version, "0.5.3");
        // Своей системы: на Mac — версионный пакет, а не «последний» klick-macos.pkg.
        let want = if cfg!(target_os = "macos") { ("https://x/pkg", 56) } else { ("https://x/exe", 40) };
        assert_eq!(r.installer.as_ref().map(|a| (a.url.as_str(), a.size)), Some(want));
        assert!(r.signature.is_some());
        assert_eq!(r.installable(), SUPPORTED);
        // Установщик другой версии под видом этой не подходит.
        let other = json!({ "tag_name": "v0.5.4", "assets": v["assets"] });
        assert!(parse_release(&other).installer.is_none());
    }

    #[test]
    fn signature_binds_key_name_and_digest() {
        let name = "klick-0.5.3-x64-setup.exe";
        let digest = "5c5ee43947011581597991e94798889e3cc818a22b9f166a439ef580fbe1aa31";
        let sig = include_str!("../tests/update-test.sig");
        assert!(signature_ok(TEST_KEY, name, digest, sig));
        assert!(!signature_ok(TEST_KEY, "klick-0.5.2-x64-setup.exe", digest, sig), "старый файл под новым именем");
        assert!(!signature_ok(TEST_KEY, name, &digest.replace('5', "6"), sig), "другое содержимое");
        assert!(!signature_ok(UPDATE_KEY, name, digest, sig), "чужой ключ");
        assert!(!signature_ok(TEST_KEY, name, digest, "не base64"));
        assert!(!signature_ok(TEST_KEY, name, digest, ""));
    }
}
