//! Что движку даёт Android: туннель (VpnService в процессе `:tunnel`), ID устройства — через Kotlin
//! (`Bridge.kt`, статические методы), проверочное ядро mihomo — прямо из `libklickcore.so` (C ABI
//! `core/export.go`). Библиотеку ядра Kotlin загружает раньше движка, здесь её функции берутся через
//! `dlsym`: так сборка движка не зависит от сборки ядра.

use crate::{bridge_class, java_string, with_env};
use jni::objects::JClass;
use klick_service::ios::Host;
use std::ffi::{c_char, c_int, CStr, CString};
use std::sync::OnceLock;

/// Метод `Bridge`, который возвращает строку ошибки или null: null — получилось.
fn bridge_result(method: &str) -> Result<(), String> {
    let class = bridge_class().ok_or("движок не связан с приложением")?;
    let answer = with_env(|env| {
        let class: &JClass = class.as_obj().into();
        let obj = env.call_static_method(class, method, "()Ljava/lang/String;", &[])?.l()?;
        java_string(env, obj)
    })
    .ok_or_else(|| format!("{method}: Kotlin не ответил"))?;
    answer.map_or(Ok(()), Err)
}

fn bridge_bool(method: &str) -> bool {
    let Some(class) = bridge_class() else { return false };
    with_env(|env| {
        let class: &JClass = class.as_obj().into();
        env.call_static_method(class, method, "()Z", &[])?.z()
    })
    .unwrap_or(false)
}

fn bridge_string(method: &str) -> Option<String> {
    let class = bridge_class()?;
    with_env(|env| {
        let class: &JClass = class.as_obj().into();
        let obj = env.call_static_method(class, method, "()Ljava/lang/String;", &[])?.l()?;
        java_string(env, obj)
    })
    .flatten()
}

// ── Ядро mihomo в процессе приложения ──────────────────────

type StartFn = unsafe extern "C" fn(*const c_char, *const c_char, c_int) -> *mut c_char;
type StopFn = unsafe extern "C" fn();
type FreeFn = unsafe extern "C" fn(*mut c_char);

struct Core {
    start: StartFn,
    stop: StopFn,
    free: FreeFn,
}

/// Функции `libklickcore.so`: библиотека уже загружена (`System.loadLibrary("klickcore")`), `dlopen` берёт её.
fn core() -> Result<&'static Core, String> {
    static CORE: OnceLock<Result<Core, String>> = OnceLock::new();
    CORE.get_or_init(|| unsafe {
        let lib = libc::dlopen(c"libklickcore.so".as_ptr(), libc::RTLD_NOW);
        if lib.is_null() {
            return Err("нет libklickcore.so".into());
        }
        let sym = |name: &CStr| {
            let p = libc::dlsym(lib, name.as_ptr());
            if p.is_null() {
                Err(format!("нет {} в libklickcore.so", name.to_string_lossy()))
            } else {
                Ok(p)
            }
        };
        Ok(Core {
            start: std::mem::transmute::<*mut libc::c_void, StartFn>(sym(c"klickcore_start")?),
            stop: std::mem::transmute::<*mut libc::c_void, StopFn>(sym(c"klickcore_stop")?),
            free: std::mem::transmute::<*mut libc::c_void, FreeFn>(sym(c"klickcore_free")?),
        })
    })
    .as_ref()
    .map_err(Clone::clone)
}

fn core_start(home: &str, config: &str) -> Result<(), String> {
    let core = core()?;
    let (home, config) = (CString::new(home).map_err(|e| e.to_string())?, CString::new(config).map_err(|e| e.to_string())?);
    // -1: без адаптера — это проверочное ядро в приложении.
    let err = unsafe { (core.start)(home.as_ptr(), config.as_ptr(), -1) };
    if err.is_null() {
        return Ok(());
    }
    let text = unsafe { CStr::from_ptr(err) }.to_string_lossy().into_owned();
    unsafe { (core.free)(err) };
    Err(text)
}

fn core_stop() {
    if let Ok(core) = core() {
        unsafe { (core.stop)() };
    }
}

pub fn host() -> Host {
    Host {
        tunnel_start: || bridge_result("tunnelStart"),
        tunnel_reload: || bridge_result("tunnelReload"),
        tunnel_stop: || {
            let _ = bridge_result("tunnelStop");
        },
        tunnel_connected: || bridge_bool("tunnelConnected"),
        core_start,
        core_stop,
        device_id: || bridge_string("deviceId"),
    }
}
