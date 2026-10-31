package app.klick.android.ui

import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.Immutable
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import app.klick.android.Appearance

/**
 * Тема — как ui/src/lib/theme.ts и iOS (`Palette`, `K` в Theme.swift): основа (палитра) и акцент. Хранит её
 * движок (`settings.appearance`). Имена цветов — те же, что CSS-переменные веб-версии.
 */
object Palette {
    val palettes: Map<String, Map<String, Long>> = mapOf(
        "graphite" to mapOf(
            "win" to 0x1a1a1d, "card" to 0x222226, "cardHover" to 0x26262b, "hover" to 0x2a2a2f, "elem" to 0x2e2e33, "raised" to 0x3a3a40,
            "ring" to 0x4a4a50, "line" to 0x2c2c31, "text" to 0xf2f2f4, "text2" to 0xc7c7cc, "text3" to 0xa1a1a8, "dim" to 0x8e8e93,
            "dim2" to 0x6e6e73, "dots" to 0x3c3c42,
        ),
        "midnight" to mapOf(
            "win" to 0x121521, "card" to 0x1a1e2d, "cardHover" to 0x1e2334, "hover" to 0x222739, "elem" to 0x262c40, "raised" to 0x333a52,
            "ring" to 0x454d68, "line" to 0x242a3c, "text" to 0xeef0f7, "text2" to 0xc3c8d8, "text3" to 0x9aa1b7, "dim" to 0x8990a8,
            "dim2" to 0x676e86, "dots" to 0x30364a,
        ),
        "oled" to mapOf(
            "win" to 0x000000, "card" to 0x111113, "cardHover" to 0x161618, "hover" to 0x1a1a1d, "elem" to 0x1e1e21, "raised" to 0x2c2c30,
            "ring" to 0x3e3e44, "line" to 0x1c1c1f, "text" to 0xf5f5f7, "text2" to 0xc7c7cc, "text3" to 0xa1a1a8, "dim" to 0x8e8e93,
            "dim2" to 0x6e6e73, "dots" to 0x2a2a2e,
        ),
        "light" to mapOf(
            "win" to 0xf5f5f7, "card" to 0xffffff, "cardHover" to 0xf7f7f9, "hover" to 0xefeff2, "elem" to 0xededf1, "raised" to 0xe3e3e8,
            "ring" to 0xc7c7cc, "line" to 0xe4e4e9, "text" to 0x1c1c1e, "text2" to 0x3a3a3c, "text3" to 0x5f5f66, "dim" to 0x7c7c83,
            "dim2" to 0xa0a0a7, "dots" to 0xd2d2d8,
        ),
    )
    val accents = listOf(0x30d158L, 0x0a84ffL, 0xbf5af2L, 0xff9f0aL, 0x64d2ffL, 0xff375fL)
    val bases = listOf("graphite" to "Графит", "midnight" to "Полночь", "oled" to "OLED", "light" to "Светлая")

    /** Основа: своя тема — выбранная, иначе графит или светлая (системная — по теме Android). */
    fun base(a: Appearance, systemDark: Boolean): String = when (a.theme) {
        "custom" -> if (palettes.containsKey(a.base)) a.base else "graphite"
        "light" -> "light"
        "dark" -> "graphite"
        else -> if (systemDark) "graphite" else "light"
    }

    fun parseHex(hex: String): Long = hex.removePrefix("#").toLongOrNull(16) ?: 0x30d158

    fun lum(c: Long) = 0.2126 * (c shr 16 and 255) / 255 + 0.7152 * (c shr 8 and 255) / 255 + 0.0722 * (c and 255) / 255

    fun hex(c: Long) = "#%06x".format(c)
}

fun rgb(c: Long) = Color(0xff000000 or c)

/** Цвета темы для интерфейса: те же имена, что у веб-версии и iOS. */
@Immutable
data class KColors(
    val win: Color, val card: Color, val cardHover: Color, val hover: Color, val elem: Color, val raised: Color, val ring: Color,
    val line: Color, val text: Color, val text2: Color, val text3: Color, val dim: Color, val dim2: Color, val dots: Color,
    val accent: Color, val onAccent: Color, val light: Boolean, val glass: Boolean,
) {
    /** Состояния — свои цвета при любой теме: «работает» и быстрый сервер зелёные, даже если тема розовая. */
    val red = rgb(0xff6961)
    val green = rgb(0x30d158)
    val orange = rgb(0xffb340)

    /** Задержка: до 80 мс — зелёная, до 160 — обычная, дальше — оранжевая, нет ответа — красная. */
    fun ping(delay: Int?): Color = when {
        delay == null -> red
        delay < 80 -> green
        delay < 160 -> text
        else -> orange
    }

    /** Скругление: веб-версии — как есть; в iOS-виде (со стеклом) — крупнее. Мелкие (меньше 9) не меняются. */
    fun r(web: Int): Dp {
        if (!glass || web < 9) return web.dp
        return when {
            web >= 20 -> 26.dp
            web >= 18 -> 24.dp
            web >= 16 -> 22.dp
            web >= 14 -> 20.dp
            web >= 12 -> 16.dp
            else -> (web + 3).dp
        }
    }

    companion object {
        fun of(a: Appearance, systemDark: Boolean): KColors {
            val base = Palette.base(a, systemDark)
            val p = Palette.palettes.getValue(base)
            val accent = if (a.theme == "custom") Palette.parseHex(a.accent) else if (base == "light") 0x1fa34aL else 0x30d158L
            val onAccent = if (base == "light" || Palette.lum(accent) < 0.45) Color.White else rgb(0x0b0b0c)
            fun t(n: String) = rgb(p.getValue(n))
            return KColors(
                t("win"), t("card"), t("cardHover"), t("hover"), t("elem"), t("raised"), t("ring"), t("line"), t("text"), t("text2"),
                t("text3"), t("dim"), t("dim2"), t("dots"), rgb(accent), onAccent, base == "light", a.glass,
            )
        }
    }
}

val LocalK = staticCompositionLocalOf { KColors.of(Appearance(), true) }

/** Цвета темы в любом месте интерфейса: `K.card`, `K.accent`… */
val K: KColors
    @Composable get() = LocalK.current

@Composable
fun KlickTheme(appearance: Appearance, content: @Composable () -> Unit) {
    CompositionLocalProvider(LocalK provides KColors.of(appearance, isSystemInDarkTheme()), content = content)
}
