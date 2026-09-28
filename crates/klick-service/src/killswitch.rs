//! Kill Switch: постоянные фильтры WFP по программам.
//!
//! На каждый exe из папки программы, на входящие и исходящие соединения IPv4 и IPv6:
//! разрешить через адаптер kl!ck, разрешить внутри компьютера, разрешить локальную сеть,
//! запретить остальное. Фильтры переживают падение службы и перезагрузку.

use crate::win::wide;
use anyhow::{bail, Context, Result};
use std::ffi::c_void;
use std::path::{Path, PathBuf};
use std::ptr::null_mut;
use windows::core::{GUID, PCWSTR, PWSTR};
use windows::Win32::Foundation::HANDLE;
use windows::Win32::NetworkManagement::WindowsFilteringPlatform::*;
use windows::Win32::System::Rpc::RPC_C_AUTHN_WINNT;

/// Провайдер и подуровень kl!ck в WFP. По ним фильтры находятся и удаляются.
const PROVIDER_KEY: GUID = GUID::from_u128(0x6b6c6963_6b00_4b53_9000_000000000001);
const SUBLAYER_KEY: GUID = GUID::from_u128(0x6b6c6963_6b00_4b53_9000_000000000002);

const FWP_E_ALREADY_EXISTS: u32 = 0x8032_0009;

const WEIGHT_BLOCK: u8 = 1;
const WEIGHT_PERMIT: u8 = 10;

const LAN_V4: [(u32, u32); 6] = [
    (0x0A00_0000, 0xFF00_0000), // 10.0.0.0/8
    (0xAC10_0000, 0xFFF0_0000), // 172.16.0.0/12
    (0xC0A8_0000, 0xFFFF_0000), // 192.168.0.0/16
    (0xA9FE_0000, 0xFFFF_0000), // 169.254.0.0/16
    (0xE000_0000, 0xF000_0000), // 224.0.0.0/4
    (0xFFFF_FFFF, 0xFFFF_FFFF), // 255.255.255.255
];
const LAN_V6: [([u8; 16], u8); 3] = [
    ([0xfe, 0x80, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0], 10),
    ([0xfc, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0], 7),
    ([0xff, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0], 8),
];

/// Все exe в папке программы, включая версии в подпапках (`app-1.0.9170`, `versions\…`).
pub fn scan_folder(folder: &Path) -> Vec<PathBuf> {
    fn walk(dir: &Path, depth: u8, out: &mut Vec<PathBuf>) {
        if depth > 6 || out.len() >= 500 {
            return;
        }
        let Ok(entries) = std::fs::read_dir(dir) else { return };
        for e in entries.flatten() {
            let Ok(ft) = e.file_type() else { continue };
            let path = e.path();
            if ft.is_dir() {
                walk(&path, depth + 1, out);
            } else if ft.is_file() && path.extension().is_some_and(|x| x.eq_ignore_ascii_case("exe")) {
                out.push(path);
            }
        }
    }
    let mut out = Vec::new();
    walk(folder, 0, &mut out);
    out.sort();
    out
}

#[derive(Debug, Default)]
pub struct ApplyReport {
    pub protected: usize,
    pub failed: Vec<PathBuf>,
    pub removed: usize,
}

pub struct Wfp {
    engine: HANDLE,
}

unsafe impl Send for Wfp {}

fn check(rc: u32, what: &str) -> Result<()> {
    if rc == 0 {
        Ok(())
    } else {
        bail!("{what}: 0x{rc:08x}")
    }
}

impl Wfp {
    pub fn open() -> Result<Self> {
        let mut engine = HANDLE::default();
        let session = FWPM_SESSION0::default();
        let rc = unsafe { FwpmEngineOpen0(PCWSTR::null(), RPC_C_AUTHN_WINNT, None, Some(&session), &mut engine) };
        check(rc, "FwpmEngineOpen0").context("нет доступа к брандмауэру: нужны права администратора")?;
        Ok(Wfp { engine })
    }

    /// Приводит брандмауэр к нужному состоянию одной транзакцией.
    pub fn apply(&self, exes: &[PathBuf], tun_luid: Option<u64>) -> Result<ApplyReport> {
        unsafe {
            check(FwpmTransactionBegin0(self.engine, 0), "FwpmTransactionBegin0")?;
            let result = self.apply_in_tx(exes, tun_luid);
            match result {
                Ok(report) => {
                    check(FwpmTransactionCommit0(self.engine), "FwpmTransactionCommit0")?;
                    Ok(report)
                }
                Err(e) => {
                    let _ = FwpmTransactionAbort0(self.engine);
                    Err(e)
                }
            }
        }
    }

    unsafe fn apply_in_tx(&self, exes: &[PathBuf], tun_luid: Option<u64>) -> Result<ApplyReport> {
        self.ensure_provider()?;
        let mut report = ApplyReport { removed: self.delete_filters()?, ..Default::default() };
        for exe in exes {
            match self.add_program(exe, tun_luid) {
                Ok(()) => report.protected += 1,
                Err(e) => {
                    tracing::warn!("Kill Switch не поставил фильтр на {}: {e:#}", exe.display());
                    report.failed.push(exe.clone());
                }
            }
        }
        Ok(report)
    }

    /// Убирает всё, что ставил kl!ck: фильтры, подуровень, провайдера.
    pub fn clear(&self) -> Result<usize> {
        unsafe {
            check(FwpmTransactionBegin0(self.engine, 0), "FwpmTransactionBegin0")?;
            let removed = match self.delete_filters() {
                Ok(n) => n,
                Err(e) => {
                    let _ = FwpmTransactionAbort0(self.engine);
                    return Err(e);
                }
            };
            let _ = FwpmSubLayerDeleteByKey0(self.engine, &SUBLAYER_KEY);
            let _ = FwpmProviderDeleteByKey0(self.engine, &PROVIDER_KEY);
            check(FwpmTransactionCommit0(self.engine), "FwpmTransactionCommit0")?;
            Ok(removed)
        }
    }

    unsafe fn ensure_provider(&self) -> Result<()> {
        let mut name = wide("kl!ck");
        let mut desc = wide("kl!ck Kill Switch");
        let provider = FWPM_PROVIDER0 {
            providerKey: PROVIDER_KEY,
            displayData: FWPM_DISPLAY_DATA0 { name: PWSTR(name.as_mut_ptr()), description: PWSTR(desc.as_mut_ptr()) },
            flags: FWPM_PROVIDER_FLAG_PERSISTENT,
            ..Default::default()
        };
        let rc = FwpmProviderAdd0(self.engine, &provider, None);
        if rc != 0 && rc != FWP_E_ALREADY_EXISTS {
            check(rc, "FwpmProviderAdd0")?;
        }
        let mut provider_key = PROVIDER_KEY;
        let sublayer = FWPM_SUBLAYER0 {
            subLayerKey: SUBLAYER_KEY,
            displayData: FWPM_DISPLAY_DATA0 { name: PWSTR(name.as_mut_ptr()), description: PWSTR(desc.as_mut_ptr()) },
            flags: FWPM_SUBLAYER_FLAG_PERSISTENT,
            providerKey: &mut provider_key,
            weight: 0x8000,
            ..Default::default()
        };
        let rc = FwpmSubLayerAdd0(self.engine, &sublayer, None);
        if rc != 0 && rc != FWP_E_ALREADY_EXISTS {
            check(rc, "FwpmSubLayerAdd0")?;
        }
        Ok(())
    }

    unsafe fn delete_filters(&self) -> Result<usize> {
        let mut ids = Vec::new();
        for layer in layers() {
            let mut provider_key = PROVIDER_KEY;
            let template = FWPM_FILTER_ENUM_TEMPLATE0 {
                providerKey: &mut provider_key,
                layerKey: layer,
                enumType: FWP_FILTER_ENUM_OVERLAPPING,
                actionMask: 0xFFFF_FFFF,
                ..Default::default()
            };
            let mut handle = HANDLE::default();
            check(FwpmFilterCreateEnumHandle0(self.engine, Some(&template), &mut handle), "FwpmFilterCreateEnumHandle0")?;
            loop {
                let mut entries: *mut *mut FWPM_FILTER0 = null_mut();
                let mut n = 0u32;
                let rc = FwpmFilterEnum0(self.engine, handle, 256, &mut entries, &mut n);
                if rc != 0 {
                    let _ = FwpmFilterDestroyEnumHandle0(self.engine, handle);
                    check(rc, "FwpmFilterEnum0")?;
                }
                if n > 0 {
                    for f in std::slice::from_raw_parts(entries, n as usize) {
                        ids.push((**f).filterId);
                    }
                }
                if !entries.is_null() {
                    let mut p = entries as *mut c_void;
                    FwpmFreeMemory0(&mut p);
                }
                if n < 256 {
                    break;
                }
            }
            let _ = FwpmFilterDestroyEnumHandle0(self.engine, handle);
        }
        for id in &ids {
            check(FwpmFilterDeleteById0(self.engine, *id), "FwpmFilterDeleteById0")?;
        }
        Ok(ids.len())
    }

    unsafe fn add_program(&self, exe: &Path, tun_luid: Option<u64>) -> Result<()> {
        let path = wide(&exe.to_string_lossy());
        let mut app_id: *mut FWP_BYTE_BLOB = null_mut();
        check(FwpmGetAppIdFromFileName0(PCWSTR(path.as_ptr()), &mut app_id), "FwpmGetAppIdFromFileName0")?;
        let result = (|| -> Result<()> {
            for layer in layers() {
                let v6 = layer == FWPM_LAYER_ALE_AUTH_CONNECT_V6 || layer == FWPM_LAYER_ALE_AUTH_RECV_ACCEPT_V6;
                let app = condition_app(app_id);

                // 1. Через адаптер kl!ck — можно.
                if let Some(luid) = tun_luid {
                    let mut luid = luid;
                    let conds = [app, condition_interface(&mut luid)];
                    self.add_filter(layer, &conds, FWP_ACTION_PERMIT, WEIGHT_PERMIT, "tun", exe)?;
                }
                // 2. Внутри компьютера — можно (порт 7890 в режиме системного прокси).
                let conds = [app, condition_loopback()];
                self.add_filter(layer, &conds, FWP_ACTION_PERMIT, WEIGHT_PERMIT, "loopback", exe)?;
                // 3. Локальная сеть — можно.
                let mut v4: Vec<FWP_V4_ADDR_AND_MASK> = LAN_V4.iter().map(|(addr, mask)| FWP_V4_ADDR_AND_MASK { addr: *addr, mask: *mask }).collect();
                let mut v6s: Vec<FWP_V6_ADDR_AND_MASK> = LAN_V6.iter().map(|(addr, len)| FWP_V6_ADDR_AND_MASK { addr: *addr, prefixLength: *len }).collect();
                let mut conds = vec![app];
                if v6 {
                    conds.extend(v6s.iter_mut().map(|m| condition_v6(m)));
                } else {
                    conds.extend(v4.iter_mut().map(|m| condition_v4(m)));
                }
                self.add_filter(layer, &conds, FWP_ACTION_PERMIT, WEIGHT_PERMIT, "lan", exe)?;
                // 4. Остальное — нельзя.
                self.add_filter(layer, &[app], FWP_ACTION_BLOCK, WEIGHT_BLOCK, "block", exe)?;
            }
            Ok(())
        })();
        let mut p = app_id as *mut c_void;
        FwpmFreeMemory0(&mut p);
        result
    }

    unsafe fn add_filter(
        &self,
        layer: GUID,
        conditions: &[FWPM_FILTER_CONDITION0],
        action: FWP_ACTION_TYPE,
        weight: u8,
        kind: &str,
        exe: &Path,
    ) -> Result<()> {
        let mut name = wide(&format!("kl!ck Kill Switch · {kind}"));
        let mut desc = wide(&exe.to_string_lossy());
        let mut provider_key = PROVIDER_KEY;
        let mut conds = conditions.to_vec();
        let filter = FWPM_FILTER0 {
            displayData: FWPM_DISPLAY_DATA0 { name: PWSTR(name.as_mut_ptr()), description: PWSTR(desc.as_mut_ptr()) },
            flags: FWPM_FILTER_FLAG_PERSISTENT,
            providerKey: &mut provider_key,
            layerKey: layer,
            subLayerKey: SUBLAYER_KEY,
            weight: FWP_VALUE0 { r#type: FWP_UINT8, Anonymous: FWP_VALUE0_0 { uint8: weight } },
            numFilterConditions: conds.len() as u32,
            filterCondition: conds.as_mut_ptr(),
            action: FWPM_ACTION0 { r#type: action, ..Default::default() },
            ..Default::default()
        };
        check(FwpmFilterAdd0(self.engine, &filter, None, None), "FwpmFilterAdd0")
    }
}

impl Drop for Wfp {
    fn drop(&mut self) {
        unsafe {
            let _ = FwpmEngineClose0(self.engine);
        }
    }
}

fn layers() -> [GUID; 4] {
    [
        FWPM_LAYER_ALE_AUTH_CONNECT_V4,
        FWPM_LAYER_ALE_AUTH_CONNECT_V6,
        FWPM_LAYER_ALE_AUTH_RECV_ACCEPT_V4,
        FWPM_LAYER_ALE_AUTH_RECV_ACCEPT_V6,
    ]
}

fn condition_app(app_id: *mut FWP_BYTE_BLOB) -> FWPM_FILTER_CONDITION0 {
    FWPM_FILTER_CONDITION0 {
        fieldKey: FWPM_CONDITION_ALE_APP_ID,
        matchType: FWP_MATCH_EQUAL,
        conditionValue: FWP_CONDITION_VALUE0 { r#type: FWP_BYTE_BLOB_TYPE, Anonymous: FWP_CONDITION_VALUE0_0 { byteBlob: app_id } },
    }
}

fn condition_interface(luid: &mut u64) -> FWPM_FILTER_CONDITION0 {
    FWPM_FILTER_CONDITION0 {
        fieldKey: FWPM_CONDITION_IP_LOCAL_INTERFACE,
        matchType: FWP_MATCH_EQUAL,
        conditionValue: FWP_CONDITION_VALUE0 { r#type: FWP_UINT64, Anonymous: FWP_CONDITION_VALUE0_0 { uint64: luid } },
    }
}

fn condition_loopback() -> FWPM_FILTER_CONDITION0 {
    FWPM_FILTER_CONDITION0 {
        fieldKey: FWPM_CONDITION_FLAGS,
        matchType: FWP_MATCH_FLAGS_ALL_SET,
        conditionValue: FWP_CONDITION_VALUE0 { r#type: FWP_UINT32, Anonymous: FWP_CONDITION_VALUE0_0 { uint32: FWP_CONDITION_FLAG_IS_LOOPBACK } },
    }
}

fn condition_v4(mask: &mut FWP_V4_ADDR_AND_MASK) -> FWPM_FILTER_CONDITION0 {
    FWPM_FILTER_CONDITION0 {
        fieldKey: FWPM_CONDITION_IP_REMOTE_ADDRESS,
        matchType: FWP_MATCH_EQUAL,
        conditionValue: FWP_CONDITION_VALUE0 { r#type: FWP_V4_ADDR_MASK, Anonymous: FWP_CONDITION_VALUE0_0 { v4AddrMask: mask } },
    }
}

fn condition_v6(mask: &mut FWP_V6_ADDR_AND_MASK) -> FWPM_FILTER_CONDITION0 {
    FWPM_FILTER_CONDITION0 {
        fieldKey: FWPM_CONDITION_IP_REMOTE_ADDRESS,
        matchType: FWP_MATCH_EQUAL,
        conditionValue: FWP_CONDITION_VALUE0 { r#type: FWP_V6_ADDR_MASK, Anonymous: FWP_CONDITION_VALUE0_0 { v6AddrMask: mask } },
    }
}
