package app.klick.android

import android.Manifest
import android.content.Intent
import android.graphics.Color
import android.net.Uri
import android.net.VpnService
import android.os.Build
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.SystemBarStyle
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.activity.result.contract.ActivityResultContracts
import androidx.activity.viewModels
import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import app.klick.android.ui.KlickTheme
import app.klick.android.ui.Palette
import app.klick.android.ui.Root

class MainActivity : ComponentActivity() {
    private val model: KlickModel by viewModels()

    /** Ссылка из «Поделиться» или klick://add — в поле «Добавить». */
    private var incoming by mutableStateOf<String?>(null)

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        incoming = linkFrom(intent)
        setContent {
            val dark = isSystemInDarkTheme()
            val light = Palette.base(model.appearance, dark) == "light"
            // Строка состояния и навигация — прозрачные, значки — под цвет основы темы.
            LaunchedEffect(light) {
                val bar = if (light) SystemBarStyle.light(Color.TRANSPARENT, Color.TRANSPARENT) else SystemBarStyle.dark(Color.TRANSPARENT)
                enableEdgeToEdge(bar, bar)
            }
            KlickTheme(model.appearance) {
                val notifications = rememberLauncherForActivityResult(ActivityResultContracts.RequestPermission()) { }
                var waiting by remember { mutableStateOf<String?>(null) }
                // Разрешение на VPN: Android спрашивает один раз; без него адаптер не открыть.
                val vpnConsent = rememberLauncherForActivityResult(ActivityResultContracts.StartActivityForResult()) { result ->
                    if (result.resultCode == RESULT_OK) model.power(waiting) else model.toast = Toast("Без разрешения на VPN kl!ck не подключится", tone = "bad")
                    waiting = null
                }
                LaunchedEffect(Unit) {
                    if (Build.VERSION.SDK_INT >= 33) notifications.launch(Manifest.permission.POST_NOTIFICATIONS)
                }
                Root(
                    model, incoming, onIncomingUsed = { incoming = null },
                    onPower = { viewing ->
                        val ask = if (model.isRunning) null else VpnService.prepare(this)
                        if (ask != null) {
                            waiting = viewing
                            vpnConsent.launch(ask)
                        } else {
                            model.power(viewing)
                        }
                    },
                )
            }
        }
    }

    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        linkFrom(intent)?.let { incoming = it }
    }

    private fun linkFrom(intent: Intent?): String? = when (intent?.action) {
        Intent.ACTION_SEND -> intent.getStringExtra(Intent.EXTRA_TEXT)?.trim()
        Intent.ACTION_VIEW -> intent.data?.let { uri: Uri -> uri.getQueryParameter("url") ?: uri.toString() }
        else -> null
    }
}
