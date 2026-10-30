package app.klick.android

import android.Manifest
import android.content.Intent
import android.net.Uri
import android.net.VpnService
import android.os.Build
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.activity.result.contract.ActivityResultContracts
import androidx.activity.viewModels
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import app.klick.android.ui.HomeScreen
import app.klick.android.ui.KlickTheme

class MainActivity : ComponentActivity() {
    private val model: KlickModel by viewModels()

    /** Ссылка из «Поделиться» или klick://add — в поле «Добавить». */
    private var incoming by mutableStateOf<String?>(null)

    override fun onCreate(savedInstanceState: Bundle?) {
        enableEdgeToEdge()
        super.onCreate(savedInstanceState)
        incoming = linkFrom(intent)
        setContent {
            KlickTheme {
                val notifications = rememberLauncherForActivityResult(ActivityResultContracts.RequestPermission()) { }
                // Разрешение на VPN: Android спрашивает один раз; без него адаптер не открыть.
                val vpnConsent = rememberLauncherForActivityResult(ActivityResultContracts.StartActivityForResult()) { result ->
                    if (result.resultCode == RESULT_OK) model.connect() else model.message = "Без разрешения на VPN kl!ck не подключится"
                }
                LaunchedEffect(Unit) {
                    if (Build.VERSION.SDK_INT >= 33) notifications.launch(Manifest.permission.POST_NOTIFICATIONS)
                }
                HomeScreen(
                    model = model,
                    incoming = incoming,
                    onIncomingUsed = { incoming = null },
                    onConnect = {
                        val ask = VpnService.prepare(this)
                        if (ask != null) vpnConsent.launch(ask) else model.connect()
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
