//! Смена сети и пробуждение: Windows сообщает об изменениях адаптеров, страж сразу проверяет связь.
//!
//! Сообщений много и о чём угодно: о своём адаптере TUN, виртуальных адаптерах Hyper-V и WSL, метриках,
//! временных IPv6-адресах. Каждое лишнее — это проверка сервера, а неудачная проверка ведёт к
//! «Переподключаюсь». Поэтому дальше пропускаем только то, что поменяло выход в интернет:
//! адаптеры со шлюзом, их IPv4-адреса и шлюзы (как на macOS — по отпечатку сети).

use std::ffi::c_void;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::time::Duration;
use tokio::sync::mpsc;
use windows::Win32::Foundation::{BOOLEAN, ERROR_BUFFER_OVERFLOW, ERROR_SUCCESS, HANDLE};
use windows::Win32::NetworkManagement::IpHelper::{
    CancelMibChangeNotify2, GetAdaptersAddresses, NotifyIpInterfaceChange, GAA_FLAG_INCLUDE_GATEWAYS, GAA_FLAG_SKIP_ANYCAST, GAA_FLAG_SKIP_DNS_SERVER,
    GAA_FLAG_SKIP_MULTICAST, IP_ADAPTER_ADDRESSES_LH, MIB_IPINTERFACE_ROW, MIB_NOTIFICATION_TYPE,
};
use windows::Win32::NetworkManagement::Ndis::IfOperStatusUp;
use windows::Win32::Networking::WinSock::{AF_INET, AF_INET6, AF_UNSPEC, SOCKADDR, SOCKADDR_IN, SOCKADDR_IN6, SOCKET_ADDRESS};

/// `IF_TYPE_PPP` из ipifcons.h.
const IF_TYPE_PPP: u32 = 23;

unsafe extern "system" fn on_change(ctx: *const c_void, _row: *const MIB_IPINTERFACE_ROW, _kind: MIB_NOTIFICATION_TYPE) {
    if ctx.is_null() {
        return;
    }
    let tx = &*(ctx as *const mpsc::UnboundedSender<()>);
    let _ = tx.send(());
}

/// Подписка на изменения сетевых адаптеров; живёт, пока жив объект.
pub struct NetWatch {
    handle: HANDLE,
    _raw: Box<mpsc::UnboundedSender<()>>,
    task: tokio::task::JoinHandle<()>,
}

unsafe impl Send for NetWatch {}
unsafe impl Sync for NetWatch {}

impl NetWatch {
    /// В канал приходит 1, когда поменялся выход в интернет.
    pub fn start(tx: mpsc::UnboundedSender<u64>) -> anyhow::Result<Self> {
        let (raw_tx, mut raw_rx) = mpsc::unbounded_channel::<()>();
        let boxed = Box::new(raw_tx);
        let mut handle = HANDLE::default();
        let ctx = &*boxed as *const mpsc::UnboundedSender<()> as *const c_void;
        unsafe { NotifyIpInterfaceChange(AF_UNSPEC, Some(on_change), Some(ctx), BOOLEAN(0), &mut handle) }.ok()?;
        let task = tokio::spawn(async move {
            let mut last = fingerprint();
            while raw_rx.recv().await.is_some() {
                // Отпечаток — и в начале пачки, и после неё: Wi-Fi, пропавший и вернувшийся за секунду,
                // тоже смена сети (старые соединения к серверу через него уже мертвы).
                let mut changed = fingerprint() != last;
                while let Ok(Some(())) = tokio::time::timeout(Duration::from_secs(1), raw_rx.recv()).await {}
                let now = fingerprint();
                changed |= now != last;
                last = now;
                // 1 — выход в интернет сменился: соединения через прежний адрес мертвы.
                if changed && tx.send(1).is_err() {
                    break;
                }
            }
        });
        Ok(NetWatch { handle, _raw: boxed, task })
    }
}

impl Drop for NetWatch {
    fn drop(&mut self) {
        unsafe {
            let _ = CancelMibChangeNotify2(self.handle);
        }
        self.task.abort();
    }
}

/// Есть ли выход в сеть: без него (сон, выключенный Wi-Fi) страж не проверяет сервер и не переподключается.
pub fn has_network() -> bool {
    !fingerprint().is_empty()
}

/// Поднятые адаптеры со шлюзом или PPP (кроме своего TUN): LUID, IPv4-адреса и шлюзы.
fn fingerprint() -> Vec<(u64, Vec<IpAddr>)> {
    let mut out = Vec::new();
    unsafe {
        let flags = GAA_FLAG_INCLUDE_GATEWAYS | GAA_FLAG_SKIP_ANYCAST | GAA_FLAG_SKIP_MULTICAST | GAA_FLAG_SKIP_DNS_SERVER;
        let mut size: u32 = 16 * 1024;
        let mut buf: Vec<u8>;
        loop {
            buf = vec![0u8; size as usize];
            let rc = GetAdaptersAddresses(AF_UNSPEC.0 as u32, flags, None, Some(buf.as_mut_ptr() as *mut IP_ADAPTER_ADDRESSES_LH), &mut size);
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
            p = a.Next;
            // У PPPoE шлюза в списке может не быть: такое подключение — тоже выход в интернет.
            if a.OperStatus != IfOperStatusUp || (a.FirstGatewayAddress.is_null() && a.IfType != IF_TYPE_PPP) {
                continue;
            }
            if a.FriendlyName.to_string().is_ok_and(|n| n.eq_ignore_ascii_case(crate::engine::TUN_DEVICE)) {
                continue;
            }
            let mut addrs = Vec::new();
            let mut u = a.FirstUnicastAddress;
            while !u.is_null() {
                if let Some(ip @ IpAddr::V4(_)) = ip_of(&(*u).Address) {
                    addrs.push(ip);
                }
                u = (*u).Next;
            }
            let mut g = a.FirstGatewayAddress;
            while !g.is_null() {
                addrs.extend(ip_of(&(*g).Address));
                g = (*g).Next;
            }
            addrs.sort();
            out.push((a.Luid.Value, addrs));
        }
    }
    out.sort();
    out
}

unsafe fn ip_of(a: &SOCKET_ADDRESS) -> Option<IpAddr> {
    let sa = a.lpSockaddr as *const SOCKADDR;
    if sa.is_null() {
        return None;
    }
    match (*sa).sa_family {
        AF_INET => {
            let b = (*(sa as *const SOCKADDR_IN)).sin_addr.S_un.S_un_b;
            Some(IpAddr::V4(Ipv4Addr::new(b.s_b1, b.s_b2, b.s_b3, b.s_b4)))
        }
        AF_INET6 => Some(IpAddr::V6(Ipv6Addr::from((*(sa as *const SOCKADDR_IN6)).sin6_addr.u.Byte))),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    /// У машины с интернетом (и у раннера CI) есть адаптер со шлюзом, и отпечаток стабилен.
    #[test]
    fn fingerprint_sees_the_way_out() {
        let f = super::fingerprint();
        eprintln!("{f:?}");
        assert!(f.iter().any(|(_, addrs)| addrs.iter().any(|a| a.is_ipv4())), "{f:?}");
        assert_eq!(f, super::fingerprint());
    }
}
