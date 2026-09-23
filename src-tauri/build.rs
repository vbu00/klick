fn main() {
    // mihomo в режиме TUN поднимает виртуальный адаптер и маршруты, а
    // kill-switch пишет правила брандмауэра — и то и другое требует прав
    // администратора. Манифест вшивается в exe.
    #[cfg(target_os = "windows")]
    {
        // Providing a custom manifest REPLACES Tauri's default one entirely
        // rather than merging with it — the first version here only had the
        // elevation block, which silently dropped the common-controls v6
        // dependency Tauri's default manifest normally carries. Without it,
        // Windows loads the ancient system comctl32.dll (v5), which doesn't
        // export TaskDialogIndirect — used by native dialogs (rfd/WebView2) —
        // and the app fails to even start with "entry point not found".
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
      <assemblyIdentity
        type="win32"
        name="Microsoft.Windows.Common-Controls"
        version="6.0.0.0"
        processorArchitecture="*"
        publicKeyToken="6595b64144ccf1df"
        language="*" />
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
        tauri_build::try_build(tauri_build::Attributes::new().windows_attributes(attrs))
            .expect("failed to run tauri-build");
    }
    #[cfg(not(target_os = "windows"))]
    {
        tauri_build::build();
    }
}
