//! Признак `mobile` — iOS и Android: движок работает в приложении, туннель держит система (расширение
//! Network Extension на iOS, VpnService на Android), а не служба с правами администратора.
fn main() {
    println!("cargo::rustc-check-cfg=cfg(mobile)");
    let os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    if os == "ios" || os == "android" {
        println!("cargo::rustc-cfg=mobile");
    }
}
