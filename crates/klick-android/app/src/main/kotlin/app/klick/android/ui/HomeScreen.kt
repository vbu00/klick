package app.klick.android.ui

import androidx.compose.animation.animateColorAsState
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.ContentPaste
import androidx.compose.material.icons.filled.PowerSettingsNew
import androidx.compose.material.icons.filled.Speed
import androidx.compose.material3.Button
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.FilterChip
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.RadioButton
import androidx.compose.material3.Scaffold
import androidx.compose.material3.SnackbarHost
import androidx.compose.material3.SnackbarHostState
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TopAppBar
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableDoubleStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.hapticfeedback.HapticFeedbackType
import androidx.compose.ui.platform.LocalClipboardManager
import androidx.compose.ui.platform.LocalHapticFeedback
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import app.klick.android.KlickModel
import app.klick.android.Srv
import app.klick.android.Texts
import kotlinx.coroutines.delay

/**
 * Главная: кнопка питания с состоянием, подписки и серверы с задержкой, «Добавить», если подписок нет.
 * Первая версия порта — проверить на телефоне движок и туннель; остальное (настройки, правила, плитка,
 * виджеты, раздельный туннель) — следующими шагами.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun HomeScreen(model: KlickModel, incoming: String?, onIncomingUsed: () -> Unit, onConnect: () -> Unit) {
    val snackbar = remember { SnackbarHostState() }
    LaunchedEffect(model.message) {
        model.message?.let {
            snackbar.showSnackbar(it)
            model.message = null
        }
    }
    Scaffold(
        topBar = { TopAppBar(title = { Text("kl!ck", fontWeight = FontWeight.Bold) }) },
        snackbarHost = { SnackbarHost(snackbar) },
    ) { pad ->
        if (!model.ready) {
            Box(Modifier.fillMaxSize().padding(pad), contentAlignment = Alignment.Center) { CircularProgressIndicator() }
            return@Scaffold
        }
        LazyColumn(
            modifier = Modifier.fillMaxSize().padding(pad),
            contentPadding = PaddingValues(16.dp),
            verticalArrangement = Arrangement.spacedBy(16.dp),
        ) {
            item { Power(model, onConnect) }
            if (model.connections.isEmpty() || incoming != null) {
                item { AddCard(model, incoming, onIncomingUsed) }
            }
            if (model.connections.size > 1) {
                item {
                    LazyRow(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                        items(model.connections, key = { it.id }) { c ->
                            FilterChip(selected = c.id == model.active, onClick = { model.selectConnection(c.id) }, label = { Text(c.name) })
                        }
                    }
                }
            }
            if (model.servers.isNotEmpty()) {
                item {
                    Row(verticalAlignment = Alignment.CenterVertically) {
                        Text("Серверы", style = MaterialTheme.typography.titleMedium, modifier = Modifier.weight(1f))
                        TextButton(onClick = model::testLatency, enabled = !model.pinging) {
                            Icon(Icons.Filled.Speed, null, Modifier.size(18.dp))
                            Spacer(Modifier.width(6.dp))
                            Text(if (model.pinging) "Проверяю…" else "Задержка")
                        }
                    }
                }
                items(model.servers, key = { it.name }) { s -> ServerRow(s, model.pinging) { model.selectServer(s.name) } }
            }
        }
    }
}

@Composable
private fun Power(model: KlickModel, onConnect: () -> Unit) {
    val haptics = LocalHapticFeedback.current
    val on = model.vpn != "off" && model.vpn != "error"
    val target = when (model.vpn) {
        "connected" -> KlickGreen
        "connecting", "reconnecting" -> KlickOrange
        "server_down", "error" -> KlickRed
        else -> MaterialTheme.colorScheme.surfaceContainerHigh
    }
    val color by animateColorAsState(target, label = "power")
    var now by remember { mutableDoubleStateOf(System.currentTimeMillis() / 1000.0) }
    LaunchedEffect(model.vpn) {
        while (model.vpn == "connected") {
            now = System.currentTimeMillis() / 1000.0
            delay(1000)
        }
    }
    Column(Modifier.fillMaxWidth().padding(top = 24.dp, bottom = 8.dp), horizontalAlignment = Alignment.CenterHorizontally) {
        Surface(
            shape = CircleShape,
            color = color,
            modifier = Modifier.size(168.dp).clickable(enabled = !model.busy) {
                haptics.performHapticFeedback(HapticFeedbackType.LongPress)
                if (on) model.disconnect() else onConnect()
            },
        ) {
            Box(contentAlignment = Alignment.Center) {
                Icon(
                    Icons.Filled.PowerSettingsNew, contentDescription = if (on) "Отключить" else "Подключить",
                    tint = if (model.vpn == "off") MaterialTheme.colorScheme.onSurfaceVariant else MaterialTheme.colorScheme.surface,
                    modifier = Modifier.size(72.dp),
                )
            }
        }
        Spacer(Modifier.height(20.dp))
        Text(Texts.status(model.vpn), style = MaterialTheme.typography.titleLarge)
        val sub = when {
            model.vpn == "connected" -> listOfNotNull(model.server, Texts.timer(model.since, now)).joinToString(" · ")
            else -> model.server ?: model.connections.firstOrNull { it.id == model.active }?.name.orEmpty()
        }
        if (sub.isNotEmpty()) Text(sub, color = MaterialTheme.colorScheme.onSurfaceVariant, fontSize = 14.sp)
    }
}

@Composable
private fun AddCard(model: KlickModel, incoming: String?, onIncomingUsed: () -> Unit) {
    var text by rememberSaveable { mutableStateOf("") }
    LaunchedEffect(incoming) {
        if (incoming != null) {
            text = incoming
            onIncomingUsed()
        }
    }
    val clipboard = LocalClipboardManager.current
    Card(colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.surfaceContainer)) {
        Column(Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
            Text("Добавить подписку", style = MaterialTheme.typography.titleMedium)
            Text(
                "Вставьте ссылку от провайдера VPN: https://…, vless://, hysteria2:// — тип определится сам.",
                style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
            OutlinedTextField(
                value = text, onValueChange = { text = it }, singleLine = true, modifier = Modifier.fillMaxWidth(),
                placeholder = { Text("Ссылка на подписку") },
                trailingIcon = {
                    IconButton(onClick = { clipboard.getText()?.text?.let { text = it.trim() } }) { Icon(Icons.Filled.ContentPaste, "Вставить") }
                },
            )
            Button(onClick = { model.add(text); text = "" }, enabled = text.isNotBlank() && !model.busy, modifier = Modifier.fillMaxWidth()) {
                Text("Добавить")
            }
        }
    }
}

@Composable
private fun ServerRow(s: Srv, pinging: Boolean, onPick: () -> Unit) {
    Card(
        onClick = onPick,
        colors = CardDefaults.cardColors(
            containerColor = if (s.selected) MaterialTheme.colorScheme.secondaryContainer else MaterialTheme.colorScheme.surfaceContainer,
        ),
    ) {
        Row(Modifier.fillMaxWidth().padding(horizontal = 8.dp, vertical = 6.dp), verticalAlignment = Alignment.CenterVertically) {
            RadioButton(selected = s.selected, onClick = onPick)
            Column(Modifier.weight(1f)) {
                Text(s.name, maxLines = 1, overflow = TextOverflow.Ellipsis, fontWeight = FontWeight.Medium)
                Text(Texts.protocol(s.kind), fontSize = 12.sp, color = MaterialTheme.colorScheme.onSurfaceVariant)
            }
            Text(
                if (pinging) "…" else Texts.ping(s.delay), color = if (pinging) MaterialTheme.colorScheme.onSurfaceVariant else pingColor(s.delay),
                fontSize = 14.sp, modifier = Modifier.padding(end = 8.dp),
            )
        }
    }
}
