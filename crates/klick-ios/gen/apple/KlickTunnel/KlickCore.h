// Ядро mihomo для расширения (crates/klick-ios/core/export.go). Строку ошибки (или NULL) освобождает
// klickcore_free.

/// tun_fd: 0 — ядро само находит дескриптор utun туннеля.
char *klickcore_start(const char *home, const char *config, int tun_fd);
char *klickcore_reload(void);
void klickcore_stop(void);
void klickcore_network_changed(void);
void klickcore_free(char *p);
