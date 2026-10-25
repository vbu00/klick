// Движок kl!ck (Rust, crates/klick-ios/src/host.rs) для Swift.
#include <stdbool.h>

/// Туннель подключился или остановился; `by_user` — его выключили в Настройках или его место занял другой VPN.
void klick_tunnel_changed(bool connected, bool by_user);
