// Установщик вшивает в себя готовый NSIS-установщик kl!ck (его и запускает
// в тихом режиме) — поэтому сначала собирается основное приложение:
//   npm run build        → src-tauri/target/release/bundle/nsis/kl!ck_<версия>_x64-setup.exe
//   npm run build:setup  → этот установщик поверх него
use std::path::PathBuf;

fn main() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..").join("..");
    let conf: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(root.join("src-tauri").join("tauri.conf.json")).expect("нет src-tauri/tauri.conf.json"))
        .expect("tauri.conf.json не разобрался");
    let version = conf["version"].as_str().expect("в tauri.conf.json нет version").to_string();
    let payload = root.join("src-tauri").join("target").join("release").join("bundle").join("nsis").join(format!("kl!ck_{version}_x64-setup.exe"));
    if !payload.exists() {
        panic!("нет {} — сначала соберите kl!ck: npm run build", payload.display());
    }
    // Сколько займёт установленный kl!ck: приложение, ядро, база GeoIP.
    let release = root.join("src-tauri").join("target").join("release");
    let bin = root.join("src-tauri").join("bin");
    let size: u64 = [release.join("klick.exe"), bin.join("mihomo.exe"), bin.join("Country.mmdb")]
        .iter()
        .map(|p| std::fs::metadata(p).map(|m| m.len()).unwrap_or(0))
        .sum();
    let app_size = std::fs::metadata(release.join("klick.exe")).map(|m| m.len()).unwrap_or(0);
    println!("cargo:rustc-env=KLICK_VERSION={version}");
    println!("cargo:rustc-env=KLICK_PAYLOAD={}", payload.display());
    println!("cargo:rustc-env=KLICK_SIZE={size}");
    println!("cargo:rustc-env=KLICK_APP_SIZE={app_size}");
    println!("cargo:rustc-env=KLICK_LICENSE={}", root.join("LICENSE").display());
    println!("cargo:rerun-if-changed={}", payload.display());
    println!("cargo:rerun-if-changed={}", root.join("src-tauri").join("tauri.conf.json").display());

    // Права администратора: установка в Program Files, правила брандмауэра.
    // Свой манифест целиком заменяет манифест Tauri, поэтому зависимость от
    // Common Controls v6 повторена здесь (без неё не стартуют системные
    // диалоги — см. такой же build.rs приложения).
    #[cfg(target_os = "windows")]
    {
        let manifest = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0">
  <trustInfo xmlns="urn:schemas-microsoft-com:asm.v3">
    <security>
      <requestedPrivileges>
        <requestedExecutionLevel level="requireAdministrator" uiAccess="false" />
      </requestedPrivileges>
    </security>
  </trustInfo>
  <dependency>
    <dependentAssembly>
      <assemblyIdentity type="win32" name="Microsoft.Windows.Common-Controls" version="6.0.0.0" processorArchitecture="*" publicKeyToken="6595b64144ccf1df" language="*" />
    </dependentAssembly>
  </dependency>
  <application xmlns="urn:schemas-microsoft-com:asm.v3">
    <windowsSettings>
      <dpiAware xmlns="http://schemas.microsoft.com/SMI/2005/WindowsSettings">true</dpiAware>
      <longPathAware xmlns="http://schemas.microsoft.com/SMI/2016/WindowsSettings">true</longPathAware>
    </windowsSettings>
  </application>
</assembly>"#;
        let attrs = tauri_build::WindowsAttributes::new().app_manifest(manifest);
        tauri_build::try_build(tauri_build::Attributes::new().windows_attributes(attrs)).expect("tauri-build не отработал");
    }
    #[cfg(not(target_os = "windows"))]
    tauri_build::build();
}
