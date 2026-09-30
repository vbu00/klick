//! Мелкие обёртки над Windows: объект задания, права на канал, DPAPI, поиск адаптера.

use anyhow::{Context, Result};
use std::ffi::c_void;
use windows::core::PCWSTR;
use windows::Win32::Foundation::{CloseHandle, LocalFree, HANDLE, HLOCAL};
use windows::Win32::NetworkManagement::IpHelper::ConvertInterfaceAliasToLuid;
use windows::Win32::NetworkManagement::Ndis::NET_LUID_LH;
use windows::Win32::Security::Authorization::{ConvertStringSecurityDescriptorToSecurityDescriptorW, SDDL_REVISION_1};
use windows::Win32::Security::Cryptography::{CryptProtectData, CryptUnprotectData, CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB};
use windows::Win32::Security::{PSECURITY_DESCRIPTOR, SECURITY_ATTRIBUTES};
use windows::Win32::System::JobObjects::{
    AssignProcessToJobObject, CreateJobObjectW, JobObjectExtendedLimitInformation, SetInformationJobObject,
    JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
};

pub fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Объект задания: когда служба завершается (даже аварийно), Windows завершает ядро вместе с ней.
pub struct Job(HANDLE);

unsafe impl Send for Job {}
unsafe impl Sync for Job {}

impl Job {
    pub fn kill_on_close() -> Result<Self> {
        unsafe {
            let job = CreateJobObjectW(None, PCWSTR::null()).context("CreateJobObjectW")?;
            let mut info = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
            info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
            SetInformationJobObject(
                job,
                JobObjectExtendedLimitInformation,
                &info as *const _ as *const c_void,
                std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            )
            .context("SetInformationJobObject")?;
            Ok(Job(job))
        }
    }

    pub fn assign(&self, process: *mut c_void) -> Result<()> {
        unsafe { AssignProcessToJobObject(self.0, HANDLE(process)).context("AssignProcessToJobObject") }
    }
}

impl Drop for Job {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.0);
        }
    }
}

/// Права на канал управления в записи SDDL. Живут, пока жив объект.
pub struct PipeSecurity {
    sd: PSECURITY_DESCRIPTOR,
    attrs: SECURITY_ATTRIBUTES,
}

unsafe impl Send for PipeSecurity {}
unsafe impl Sync for PipeSecurity {}

impl PipeSecurity {
    /// Система и администраторы — полный доступ; вошедшие в Windows пользователи — чтение и запись.
    /// TODO: окно должно проверять, что на том конце канала процесс службы, а не чужой экземпляр.
    pub fn interactive_users() -> Result<Box<Self>> {
        Self::from_sddl("D:P(A;;GA;;;SY)(A;;GA;;;BA)(A;;GRGW;;;IU)")
    }

    pub fn from_sddl(sddl: &str) -> Result<Box<Self>> {
        let w = wide(sddl);
        let mut sd = PSECURITY_DESCRIPTOR::default();
        unsafe {
            ConvertStringSecurityDescriptorToSecurityDescriptorW(PCWSTR(w.as_ptr()), SDDL_REVISION_1, &mut sd, None)
                .context("ConvertStringSecurityDescriptorToSecurityDescriptorW")?;
        }
        let mut me = Box::new(PipeSecurity { sd, attrs: SECURITY_ATTRIBUTES::default() });
        me.attrs.nLength = std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32;
        me.attrs.lpSecurityDescriptor = me.sd.0;
        Ok(me)
    }

    pub fn as_ptr(&mut self) -> *mut c_void {
        &mut self.attrs as *mut SECURITY_ATTRIBUTES as *mut c_void
    }
}

impl Drop for PipeSecurity {
    fn drop(&mut self) {
        unsafe {
            let _ = LocalFree(HLOCAL(self.sd.0));
        }
    }
}

/// Шифрует данные ключом учётной записи, под которой работает процесс.
/// У службы это учётная запись системы: расшифровать может только она.
pub fn protect(data: &[u8]) -> Result<Vec<u8>> {
    unsafe {
        let input = CRYPT_INTEGER_BLOB { cbData: data.len() as u32, pbData: data.as_ptr() as *mut u8 };
        let mut out = CRYPT_INTEGER_BLOB::default();
        CryptProtectData(&input, PCWSTR::null(), None, None, None, CRYPTPROTECT_UI_FORBIDDEN, &mut out).context("CryptProtectData")?;
        let bytes = std::slice::from_raw_parts(out.pbData, out.cbData as usize).to_vec();
        let _ = LocalFree(HLOCAL(out.pbData as *mut c_void));
        Ok(bytes)
    }
}

pub fn unprotect(data: &[u8]) -> Result<Vec<u8>> {
    unsafe {
        let input = CRYPT_INTEGER_BLOB { cbData: data.len() as u32, pbData: data.as_ptr() as *mut u8 };
        let mut out = CRYPT_INTEGER_BLOB::default();
        CryptUnprotectData(&input, None, None, None, None, CRYPTPROTECT_UI_FORBIDDEN, &mut out).context("CryptUnprotectData")?;
        let bytes = std::slice::from_raw_parts(out.pbData, out.cbData as usize).to_vec();
        let _ = LocalFree(HLOCAL(out.pbData as *mut c_void));
        Ok(bytes)
    }
}

/// LUID сетевого адаптера по имени, например `klick`. Нет адаптера — `None`.
pub fn interface_luid(alias: &str) -> Option<u64> {
    let w = wide(alias);
    let mut luid = NET_LUID_LH::default();
    let rc = unsafe { ConvertInterfaceAliasToLuid(PCWSTR(w.as_ptr()), &mut luid) };
    if rc.is_ok() {
        Some(unsafe { luid.Value })
    } else {
        None
    }
}

/// Поднят ли адаптер TUN ядра.
pub fn tun_up() -> bool {
    interface_luid(crate::engine::TUN_DEVICE).is_some()
}

/// Папка данных рабочей службы: доступ только у системы и администраторов, с наследованием.
/// Там лежат ключи серверов, поэтому обычные программы пользователя их читать не должны.
pub fn restrict_to_admins(path: &std::path::Path) -> Result<()> {
    use windows::Win32::Foundation::BOOL;
    use windows::Win32::Security::Authorization::{SetNamedSecurityInfoW, SE_FILE_OBJECT};
    use windows::Win32::Security::{GetSecurityDescriptorDacl, ACL, DACL_SECURITY_INFORMATION, PROTECTED_DACL_SECURITY_INFORMATION, PSID};
    let sddl = wide("D:P(A;OICI;FA;;;SY)(A;OICI;FA;;;BA)");
    let target = wide(&path.to_string_lossy());
    unsafe {
        let mut sd = PSECURITY_DESCRIPTOR::default();
        ConvertStringSecurityDescriptorToSecurityDescriptorW(PCWSTR(sddl.as_ptr()), SDDL_REVISION_1, &mut sd, None)
            .context("ConvertStringSecurityDescriptorToSecurityDescriptorW")?;
        let mut present = BOOL(0);
        let mut defaulted = BOOL(0);
        let mut dacl: *mut ACL = std::ptr::null_mut();
        let result = GetSecurityDescriptorDacl(sd, &mut present, &mut dacl, &mut defaulted).context("GetSecurityDescriptorDacl").and_then(|_| {
            SetNamedSecurityInfoW(
                PCWSTR(target.as_ptr()),
                SE_FILE_OBJECT,
                DACL_SECURITY_INFORMATION | PROTECTED_DACL_SECURITY_INFORMATION,
                PSID::default(),
                PSID::default(),
                Some(dacl),
                None,
            )
            .ok()
            .context("SetNamedSecurityInfoW")
        });
        let _ = LocalFree(HLOCAL(sd.0));
        result
    }
}

/// Свободный порт на 127.0.0.1 для служебных входов ядра.
pub fn free_port() -> Result<u16> {
    let l = std::net::TcpListener::bind("127.0.0.1:0")?;
    Ok(l.local_addr()?.port())
}

pub fn is_elevated() -> bool {
    use windows::Win32::Security::{GetTokenInformation, TokenElevation, TOKEN_ELEVATION, TOKEN_QUERY};
    use windows::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};
    unsafe {
        let mut token = HANDLE::default();
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token).is_err() {
            return false;
        }
        let mut elevation = TOKEN_ELEVATION::default();
        let mut len = 0u32;
        let ok = GetTokenInformation(
            token,
            TokenElevation,
            Some(&mut elevation as *mut _ as *mut c_void),
            std::mem::size_of::<TOKEN_ELEVATION>() as u32,
            &mut len,
        )
        .is_ok();
        let _ = CloseHandle(token);
        ok && elevation.TokenIsElevated != 0
    }
}


/// «Windows 11 · 24H2 · x64»: номер сборки отличает 11 от 10, в названии из реестра у 11 до сих пор «10».
pub fn os_name() -> String {
    let key = r"SOFTWARE\Microsoft\Windows NT\CurrentVersion";
    let build: u32 = reg_string(key, "CurrentBuildNumber").and_then(|b| b.parse().ok()).unwrap_or(0);
    let name = if build >= 22000 { "Windows 11" } else if build > 0 { "Windows 10" } else { "Windows" };
    let arch = match std::env::consts::ARCH {
        "x86_64" => "x64",
        "aarch64" => "ARM64",
        other => other,
    };
    [Some(name.to_string()), reg_string(key, "DisplayVersion"), Some(arch.to_string())].into_iter().flatten().collect::<Vec<_>>().join(" · ")
}

fn reg_string(key: &str, value: &str) -> Option<String> {
    use windows::Win32::Foundation::ERROR_SUCCESS;
    use windows::Win32::System::Registry::{RegGetValueW, HKEY_LOCAL_MACHINE, RRF_RT_REG_SZ};
    let (k, v) = (wide(key), wide(value));
    let mut buf = [0u16; 256];
    let mut size = (buf.len() * 2) as u32;
    let rc = unsafe { RegGetValueW(HKEY_LOCAL_MACHINE, PCWSTR(k.as_ptr()), PCWSTR(v.as_ptr()), RRF_RT_REG_SZ, None, Some(buf.as_mut_ptr() as *mut c_void), Some(&mut size)) };
    if rc != ERROR_SUCCESS {
        return None;
    }
    let len = (size as usize / 2).saturating_sub(1).min(buf.len());
    Some(String::from_utf16_lossy(&buf[..len])).filter(|s| !s.is_empty())
}
