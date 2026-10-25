// Движок kl!ck (Rust, crates/klick-native) для Swift.
#include <stdbool.h>

/// Туннель подключился или остановился; `by_user` — его выключили в Настройках или его место занял другой VPN.
void klick_tunnel_changed(bool connected, bool by_user);

/// Ответ на команду или событие движка: `json` живёт только до возврата из функции.
typedef void (*klick_callback)(void *ctx, const char *json, bool ok);

/// Поднять движок. NULL — поднялся, иначе текст ошибки (освободить klick_native_free). Не с главного потока.
char *klick_native_start(void);
/// Команда движку: `{"cmd": "...", "args": {...}}`. Ответ — в `cb` с другого потока.
void klick_native_call(const char *request, void *ctx, klick_callback cb);
/// События движка — в `cb`. `false` — движок ещё не поднят.
bool klick_native_subscribe(void *ctx, klick_callback cb);
void klick_native_free(char *p);
