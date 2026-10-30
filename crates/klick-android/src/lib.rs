//! kl!ck для Android с родным интерфейсом (Kotlin/Compose, `crates/klick-android/app`). Движок службы — в
//! процессе приложения, как у iOS (`klick-native`): Kotlin зовёт те же команды движка, что и окно (JSON
//! klick-proto), и получает события. Туннель держит VpnService в своём процессе (`:tunnel`) с ядром mihomo
//! библиотекой (`libklickcore.so`); в процессе приложения то же ядро — проверочное (задержка, локальная
//! проверка серверов). Связь с Kotlin — JNI: классы `app.klick.android.Engine` (вход) и `Bridge` (туннель,
//! ID устройства). Как устроено — docs/android-port.md.

mod host;

use jni::objects::{GlobalRef, JClass, JObject, JString, JValue};
use jni::sys::{jboolean, jlong, jstring, JNI_TRUE};
use jni::{JNIEnv, JavaVM};
use klick_proto::Command;
use klick_service::engine::{self, EngineHandle};
use klick_service::paths::{Paths, Profile};
use serde_json::{json, Value};
use std::path::PathBuf;
use std::sync::OnceLock;

static RUNTIME: OnceLock<tokio::runtime::Runtime> = OnceLock::new();
static ENGINE: OnceLock<EngineHandle> = OnceLock::new();
static VM: OnceLock<JavaVM> = OnceLock::new();
/// Классы Kotlin. Находить их можно только с потока Java (у потоков Rust загрузчик классов системный),
/// поэтому — один раз при запуске движка.
static ENGINE_CLASS: OnceLock<GlobalRef> = OnceLock::new();
static BRIDGE_CLASS: OnceLock<GlobalRef> = OnceLock::new();

fn runtime() -> &'static tokio::runtime::Runtime {
    RUNTIME.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .thread_name("klick-engine")
            .build()
            .expect("tokio на Android не запустился")
    })
}

fn unreachable() -> Value {
    json!({ "code": "service.unreachable" })
}

/// Поток Rust — к Java: присоединяется к JVM один раз и до конца жизни потока.
pub(crate) fn with_env<R>(f: impl FnOnce(&mut JNIEnv) -> jni::errors::Result<R>) -> Option<R> {
    let vm = VM.get()?;
    let mut env = vm.attach_current_thread_permanently().ok()?;
    match f(&mut env) {
        Ok(r) => Some(r),
        Err(e) => {
            // Исключение Kotlin не должно уронить поток Rust: сбросить и считать, что ответа нет.
            if env.exception_check().unwrap_or(false) {
                let _ = env.exception_describe();
                let _ = env.exception_clear();
            }
            tracing::warn!("JNI: {e}");
            None
        }
    }
}

pub(crate) fn bridge_class() -> Option<&'static GlobalRef> {
    BRIDGE_CLASS.get()
}

/// Строка Java (или null) — в `Option<String>`.
pub(crate) fn java_string(env: &mut JNIEnv, obj: JObject) -> jni::errors::Result<Option<String>> {
    if obj.is_null() {
        return Ok(None);
    }
    let s = JString::from(obj);
    let text: String = env.get_string(&s)?.into();
    Ok(Some(text))
}

fn start(data: PathBuf, core: PathBuf, resources: PathBuf) -> Result<(), String> {
    klick_service::ios::init(host::host());
    let paths = Paths::ios(data, core, resources);
    paths.ensure_dirs().map_err(|e| format!("{e:#}"))?;
    let _ = klick_service::init_file_log(&paths);
    let _guard = runtime().enter();
    let (engine, _task) = engine::spawn(paths, Profile::ios()).map_err(|e| format!("{e:#}"))?;
    let _ = ENGINE.set(engine);
    Ok(())
}

/// `Engine.nativeStart(data, group, resources)`: поднять движок. null — поднялся (или уже работает),
/// иначе текст ошибки. `data` — настройки, ключи и журнал; `group` — папка, общая с процессом туннеля
/// (в ней `core/`: конфиг, серверы, наборы); `resources` — наборы и база стран, распакованные из APK.
#[no_mangle]
pub extern "system" fn Java_app_klick_android_Engine_nativeStart<'l>(
    mut env: JNIEnv<'l>,
    class: JClass<'l>,
    data: JString<'l>,
    group: JString<'l>,
    resources: JString<'l>,
) -> jstring {
    if ENGINE.get().is_some() {
        return std::ptr::null_mut();
    }
    let result = (|| -> Result<(), String> {
        let vm = env.get_java_vm().map_err(|e| e.to_string())?;
        let _ = VM.set(vm);
        let _ = ENGINE_CLASS.set(env.new_global_ref(&class).map_err(|e| e.to_string())?);
        let bridge = env.find_class("app/klick/android/Bridge").map_err(|e| e.to_string())?;
        let _ = BRIDGE_CLASS.set(env.new_global_ref(&bridge).map_err(|e| e.to_string())?);
        let path = |env: &mut JNIEnv, s: &JString| -> Result<PathBuf, String> {
            let text: String = env.get_string(s).map_err(|e| e.to_string())?.into();
            Ok(PathBuf::from(text))
        };
        let (data, group, resources) = (path(&mut env, &data)?, path(&mut env, &group)?, path(&mut env, &resources)?);
        start(data, group, resources)
    })();
    match result {
        Ok(()) => std::ptr::null_mut(),
        Err(e) => env.new_string(e).map(|s| s.into_raw()).unwrap_or(std::ptr::null_mut()),
    }
}

async fn call(text: String) -> Result<Value, Value> {
    let engine = ENGINE.get().ok_or_else(unreachable)?;
    let cmd: Command = serde_json::from_str(&text).map_err(|_| json!({ "code": "proto.bad_request" }))?;
    let result = if cmd == Command::IpCheck { engine.ip_check().await } else { engine.call(cmd).await };
    result.map_err(|e| serde_json::to_value(e).unwrap_or_else(|_| unreachable()))
}

/// `Engine.nativeCall(request, id)`: команда движку — `{"cmd": "...", "args": {...}}`, как в klick-proto.
/// Ответ — в `Engine.onReply(id, json, ok)` с другого потока: `ok` — JSON результата, иначе JSON ошибки
/// `{"code": ..., "params": ...}`.
#[no_mangle]
pub extern "system" fn Java_app_klick_android_Engine_nativeCall<'l>(mut env: JNIEnv<'l>, _class: JClass<'l>, request: JString<'l>, id: jlong) {
    let text: String = env.get_string(&request).map(Into::into).unwrap_or_default();
    runtime().spawn(async move {
        let (body, ok) = match call(text).await {
            Ok(v) => (v.to_string(), true),
            Err(e) => (e.to_string(), false),
        };
        let Some(class) = ENGINE_CLASS.get() else { return };
        with_env(|env| {
            let body = env.new_string(body)?;
            let class: &JClass = class.as_obj().into();
            env.call_static_method(class, "onReply", "(JLjava/lang/String;Z)V", &[JValue::Long(id), JValue::Object(&body), JValue::Bool(ok.into())])
                .map(|_| ())
        });
    });
}

/// `Engine.nativeSubscribe()`: события движка (состояние, скорость, уведомления, «настройки изменились») —
/// в `Engine.onEvent(json)` по одному JSON klick-proto `Event`. `false` — движок ещё не поднят.
#[no_mangle]
pub extern "system" fn Java_app_klick_android_Engine_nativeSubscribe<'l>(_env: JNIEnv<'l>, _class: JClass<'l>) -> jboolean {
    let Some(engine) = ENGINE.get() else { return 0 };
    let mut events = engine.events.subscribe();
    runtime().spawn(async move {
        use tokio::sync::broadcast::error::RecvError;
        loop {
            match events.recv().await {
                Ok(ev) => {
                    let Ok(text) = serde_json::to_string(&ev) else { continue };
                    let Some(class) = ENGINE_CLASS.get() else { continue };
                    with_env(|env| {
                        let text = env.new_string(text)?;
                        let class: &JClass = class.as_obj().into();
                        env.call_static_method(class, "onEvent", "(Ljava/lang/String;)V", &[JValue::Object(&text)]).map(|_| ())
                    });
                }
                Err(RecvError::Lagged(_)) => continue,
                Err(RecvError::Closed) => break,
            }
        }
    });
    JNI_TRUE
}

/// `Engine.nativeTunnelChanged(connected, byUser)`: служба туннеля подключилась или остановилась.
/// `byUser` — VPN выключили в Настройках, шторке или его место занял другой VPN: движок не включает его снова.
#[no_mangle]
pub extern "system" fn Java_app_klick_android_Engine_nativeTunnelChanged<'l>(_env: JNIEnv<'l>, _class: JClass<'l>, connected: jboolean, by_user: jboolean) {
    if connected == 0 {
        klick_service::ios::tunnel_stopped(by_user != 0);
        return;
    }
    // VPN включили из шторки, «Постоянного VPN» или он поднялся сам после перезагрузки, пока окно открыто.
    if let Some(engine) = ENGINE.get().cloned() {
        runtime().spawn(async move {
            let _ = engine.call(Command::Connect).await;
        });
    }
}

