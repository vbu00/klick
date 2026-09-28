//! Смена сети и пробуждение: Windows сообщает об изменениях адаптеров, страж сразу проверяет связь.

use std::ffi::c_void;
use tokio::sync::mpsc;
use windows::Win32::Foundation::{BOOLEAN, HANDLE};
use windows::Win32::NetworkManagement::IpHelper::{CancelMibChangeNotify2, NotifyIpInterfaceChange, MIB_IPINTERFACE_ROW, MIB_NOTIFICATION_TYPE};
use windows::Win32::Networking::WinSock::AF_UNSPEC;

unsafe extern "system" fn on_change(ctx: *const c_void, row: *const MIB_IPINTERFACE_ROW, _kind: MIB_NOTIFICATION_TYPE) {
    if ctx.is_null() {
        return;
    }
    let tx = &*(ctx as *const mpsc::UnboundedSender<u64>);
    let luid = if row.is_null() { 0 } else { (*row).InterfaceLuid.Value };
    let _ = tx.send(luid);
}

/// Подписка на изменения сетевых адаптеров; живёт, пока жив объект.
pub struct NetWatch {
    handle: HANDLE,
    _tx: Box<mpsc::UnboundedSender<u64>>,
}

unsafe impl Send for NetWatch {}
unsafe impl Sync for NetWatch {}

impl NetWatch {
    /// В канал приходит LUID изменившегося адаптера.
    pub fn start(tx: mpsc::UnboundedSender<u64>) -> anyhow::Result<Self> {
        let boxed = Box::new(tx);
        let mut handle = HANDLE::default();
        let ctx = &*boxed as *const mpsc::UnboundedSender<u64> as *const c_void;
        unsafe { NotifyIpInterfaceChange(AF_UNSPEC, Some(on_change), Some(ctx), BOOLEAN(0), &mut handle) }.ok()?;
        Ok(NetWatch { handle, _tx: boxed })
    }
}

impl Drop for NetWatch {
    fn drop(&mut self) {
        unsafe {
            let _ = CancelMibChangeNotify2(self.handle);
        }
    }
}
