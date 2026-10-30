package app.klick.android.ui

import android.os.Build
import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.dynamicDarkColorScheme
import androidx.compose.material3.dynamicLightColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext

/** Зелёный kl!ck — «подключено» и задержка до 80 мс, как на компьютере и iOS. */
val KlickGreen = Color(0xFF30D158)
val KlickOrange = Color(0xFFFF9F0A)
val KlickRed = Color(0xFFFF6961)

// Без Material You (Android 8–11) — «Графит» kl!ck.
private val Graphite = darkColorScheme(
    primary = KlickGreen,
    onPrimary = Color(0xFF0B0B0C),
    background = Color(0xFF101012),
    surface = Color(0xFF1A1A1D),
    surfaceVariant = Color(0xFF26262B),
    surfaceContainer = Color(0xFF222226),
    surfaceContainerHigh = Color(0xFF2A2A2F),
)
private val Light = lightColorScheme(primary = Color(0xFF1FA34A))

/** Material You: цвета из обоев на Android 12+, иначе тема kl!ck. */
@Composable
fun KlickTheme(content: @Composable () -> Unit) {
    val dark = isSystemInDarkTheme()
    val ctx = LocalContext.current
    val colors = when {
        Build.VERSION.SDK_INT >= 31 -> if (dark) dynamicDarkColorScheme(ctx) else dynamicLightColorScheme(ctx)
        dark -> Graphite
        else -> Light
    }
    MaterialTheme(colorScheme = colors, content = content)
}

/** Цвет задержки: до 80 мс — зелёный, до 200 — оранжевый, дольше или нет ответа — красный. */
fun pingColor(delay: Int?): Color = when {
    delay == null -> KlickRed
    delay < 80 -> KlickGreen
    delay < 200 -> KlickOrange
    else -> KlickRed
}
