//! Соседи на компьютере: zapret, GoodbyeDPI (драйвер WinDivert), другие VPN и прокси-ядра.
//! Они перехватывают тот же трафик, что и kl!ck, поэтому окно подсказывает, что с ними сделать.

use crate::win::wide;
use std::path::Path;
use windows::core::{PCWSTR, PWSTR};
use windows::Win32::Foundation::{CloseHandle, ERROR_BUFFER_OVERFLOW, ERROR_SUCCESS};
use windows::Win32::NetworkManagement::IpHelper::{GetAdaptersAddresses, GAA_FLAG_SKIP_ANYCAST, GAA_FLAG_SKIP_MULTICAST, IP_ADAPTER_ADDRESSES_LH};
use windows::Win32::NetworkManagement::Ndis::IfOperStatusUp;
use windows::Win32::Networking::WinSock::AF_UNSPEC;
use windows::Win32::System::Diagnostics::ToolHelp::{CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS};
use windows::Win32::System::Services::{CloseServiceHandle, OpenSCManagerW, OpenServiceW, QueryServiceStatus, SC_MANAGER_CONNECT, SERVICE_QUERY_STATUS, SERVICE_RUNNING, SERVICE_STATUS};
use windows::Win32::System::Threading::{OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION};

pub use klick_proto::NeighborView as Neighbor;

const KNOWN: [(&str, &str, &str); 17] = [
    ("winws.exe", "dpi_bypass", "zapret"),
    ("goodbyedpi.exe", "dpi_bypass", "GoodbyeDPI"),
    ("ciadpi.exe", "dpi_bypass", "ByeDPI"),
    ("byedpi.exe", "dpi_bypass", "ByeDPI"),
    ("wireguard.exe", "vpn", "WireGuard"),
    ("openvpn.exe", "vpn", "OpenVPN"),
    ("amneziavpn.exe", "vpn", "AmneziaVPN"),
    ("amneziavpn-service.exe", "vpn", "AmneziaVPN"),
    ("outline.exe", "vpn", "Outline"),
    ("sing-box.exe", "proxy_core", "sing-box"),
    ("xray.exe", "proxy_core", "Xray"),
    ("v2rayn.exe", "proxy_core", "v2rayN"),
    ("nekoray.exe", "proxy_core", "NekoRay"),
    ("nekobox.exe", "proxy_core", "NekoBox"),
    ("hiddify.exe", "proxy_core", "Hiddify"),
    ("clash-verge.exe", "proxy_core", "Clash Verge"),
    ("happ.exe", "proxy_core", "Happ"),
];

/// Что сейчас работает рядом. `own_core` — наше ядро, его не считаем.
pub fn scan(own_core: &Path) -> Vec<Neighbor> {
    let mut out = Vec::new();
    for (name, path) in processes() {
        let lower = name.to_ascii_lowercase();
        if let Some((_, kind, title)) = KNOWN.iter().find(|(exe, _, _)| *exe == lower) {
            push(&mut out, Neighbor { kind: (*kind).into(), name: (*title).into(), conflicts_with_tun: *kind == "dpi_bypass" || *kind == "vpn" });
        } else if lower == "mihomo.exe" || lower.starts_with("clash") {
            let ours = path.as_deref().is_some_and(|p| Path::new(p).eq(own_core));
            if !ours {
                push(&mut out, Neighbor { kind: "proxy_core".into(), name: name.clone(), conflicts_with_tun: false });
            }
        }
    }
    for adapter in vpn_adapters() {
        push(&mut out, Neighbor { kind: "vpn_adapter".into(), name: adapter, conflicts_with_tun: true });
    }
    for svc in ["WinDivert", "WinDivert14", "WinDivert1.4"] {
        if service_running(svc) {
            push(&mut out, Neighbor { kind: "windivert".into(), name: svc.into(), conflicts_with_tun: true });
        }
    }
    out
}

fn push(out: &mut Vec<Neighbor>, n: Neighbor) {
    if !out.contains(&n) {
        out.push(n);
    }
}

/// Имена процессов и, если удаётся, полный путь.
fn processes() -> Vec<(String, Option<String>)> {
    let mut out = Vec::new();
    unsafe {
        let Ok(snap) = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) else { return out };
        let mut entry = PROCESSENTRY32W { dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32, ..Default::default() };
        let mut ok = Process32FirstW(snap, &mut entry).is_ok();
        while ok {
            let len = entry.szExeFile.iter().position(|c| *c == 0).unwrap_or(entry.szExeFile.len());
            let name = String::from_utf16_lossy(&entry.szExeFile[..len]);
            let path = process_path(entry.th32ProcessID);
            out.push((name, path));
            ok = Process32NextW(snap, &mut entry).is_ok();
        }
        let _ = CloseHandle(snap);
    }
    out
}

pub(crate) fn process_path(pid: u32) -> Option<String> {
    unsafe {
        let h = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
        let mut buf = [0u16; 1024];
        let mut len = buf.len() as u32;
        let ok = QueryFullProcessImageNameW(h, PROCESS_NAME_WIN32, PWSTR(buf.as_mut_ptr()), &mut len).is_ok();
        let _ = CloseHandle(h);
        ok.then(|| String::from_utf16_lossy(&buf[..len as usize]))
    }
}

/// Поднятые туннельные адаптеры чужих VPN (WireGuard, TAP, Wintun других программ).
fn vpn_adapters() -> Vec<String> {
    let mut out = Vec::new();
    unsafe {
        let mut size: u32 = 16 * 1024;
        let mut buf: Vec<u8>;
        loop {
            buf = vec![0u8; size as usize];
            let rc = GetAdaptersAddresses(
                AF_UNSPEC.0 as u32,
                GAA_FLAG_SKIP_ANYCAST | GAA_FLAG_SKIP_MULTICAST,
                None,
                Some(buf.as_mut_ptr() as *mut IP_ADAPTER_ADDRESSES_LH),
                &mut size,
            );
            if rc == ERROR_BUFFER_OVERFLOW.0 {
                continue;
            }
            if rc != ERROR_SUCCESS.0 {
                return out;
            }
            break;
        }
        let mut p = buf.as_ptr() as *const IP_ADAPTER_ADDRESSES_LH;
        while !p.is_null() {
            let a = &*p;
            let name = a.FriendlyName.to_string().unwrap_or_default();
            let desc = a.Description.to_string().unwrap_or_default();
            let tunnel = ["wireguard", "tap-windows", "tap-", "wintun", "tunnel", "openvpn", "amnezia", "sing-tun"].iter().any(|k| desc.to_ascii_lowercase().contains(k));
            if a.OperStatus == IfOperStatusUp && tunnel && !name.eq_ignore_ascii_case(crate::engine::TUN_DEVICE) && !desc.to_ascii_lowercase().contains("teredo") {
                out.push(format!("{name} ({desc})"));
            }
            p = a.Next;
        }
    }
    out
}

fn service_running(name: &str) -> bool {
    unsafe {
        let Ok(scm) = OpenSCManagerW(PCWSTR::null(), PCWSTR::null(), SC_MANAGER_CONNECT) else { return false };
        let w = wide(name);
        let running = match OpenServiceW(scm, PCWSTR(w.as_ptr()), SERVICE_QUERY_STATUS) {
            Ok(svc) => {
                let mut st = SERVICE_STATUS::default();
                let r = QueryServiceStatus(svc, &mut st).is_ok() && st.dwCurrentState == SERVICE_RUNNING;
                let _ = CloseServiceHandle(svc);
                r
            }
            Err(_) => false,
        };
        let _ = CloseServiceHandle(scm);
        running
    }
}
