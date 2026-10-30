package app.klick.android

/**
 * Ядро mihomo в процессе туннеля (`libklickcore.so`, `crates/klick-ios/core/jni_android.go`).
 * Ошибка — строкой, null — всё хорошо.
 */
object TunnelCore {
    init {
        System.loadLibrary("klickcore")
    }

    /** Ядро с конфигом [config] (папка ядра — [home]) на адаптере [fd]: дескриптор переходит ядру. */
    @JvmStatic external fun start(home: String, config: String, fd: Int): String?

    /** Перечитать конфиг целиком, вместе со входом управления: адаптер не трогается. */
    @JvmStatic external fun reload(): String?

    @JvmStatic external fun stop()

    /** Wi-Fi ↔ сотовая: соединения через прежнюю сеть мертвы, закрыть их сразу, а не ждать тайм-аута. */
    @JvmStatic external fun networkChanged()
}
