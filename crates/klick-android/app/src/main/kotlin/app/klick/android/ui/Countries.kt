package app.klick.android.ui

import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.rotate
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.hapticfeedback.HapticFeedbackType
import androidx.compose.ui.platform.LocalHapticFeedback
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import app.klick.android.Srv
import app.klick.android.Texts
import java.text.Collator
import java.util.Locale

// Страна сервера по его имени из подписки и подпись для списка «по странам» — как ui/src/lib/countries.ts и
// iOS (Countries.swift). Имена придумывает панель: «🇧🇾 BY · HY2», «[NL] Amsterdam», «Германия · Frankfurt».

/** Сервер, разобранный для списка по странам. */
data class ParsedServer(
    /** Код страны ISO (`DE`); null — страну не узнали, сервер пойдёт в «Другие». */
    val code: String?,
    /** Остаток имени без страны, по-человечески: «Hysteria2 · WARP», «Amsterdam». Пусто — в имени была только страна. */
    val title: String,
    /** Короткая метка протокола для строки страны: HY2, VLESS, AWG… */
    val tag: String,
    /** Протокол уже назван в `title` — второй раз его не показываем. */
    val named: Boolean,
)

data class CountryGroup(val key: String, val code: String?, val name: String, val items: List<Pair<Srv, ParsedServer>>) {
    /** Метки протоколов без повторов, в порядке подписки. */
    val tags get() = items.map { it.second.tag }.distinct()
}

object Countries {
    private val ru = Locale("ru")
    private val en = Locale.ENGLISH

    /** Где имя системы длинное или неудачное. */
    private val ruName = mapOf("US" to "США", "GB" to "Великобритания", "AE" to "ОАЭ", "HK" to "Гонконг", "KR" to "Южная Корея", "EU" to "Евросоюз")

    /** Двухбуквенные коды, которые в начале имени чаще значат протокол, чем страну. */
    private val notCountry = setOf("SS")
    private val regions = Locale.getISOCountries().toSet()
    private fun isRegion(code: String) = code !in notCountry && code in regions

    /** Название страны по-русски: «Беларусь», «США». */
    fun name(code: String?): String {
        if (code == null) return "Другие"
        ruName[code]?.let { return it }
        val n = Locale("", code).getDisplayCountry(ru)
        return if (n.isEmpty() || n == code) code else n.replace(Regex("\\s*\\(.*\\)$"), "")
    }

    /** `DE` → 🇩🇪. */
    fun emoji(code: String): String = code.uppercase().map { String(Character.toChars(0x1F1E6 + (it - 'A'))) }.joinToString("")

    private val namedCodes = ("RU BY UA KZ UZ KG AM GE AZ MD DE NL FI FR GB US PL SE NO DK IS IE BE LU PT ES IT CH AT CZ SK SI HU RO BG RS HR GR CY MT TR IL AE SA QA " +
        "IN JP KR CN HK TW SG TH VN MY ID PH AU NZ CA MX BR AR CL ZA EG LV LT EE").split(" ")

    private val aliases = listOf(
        "США" to "US", "USA" to "US", "America" to "US", "Америка" to "US",
        "UK" to "GB", "England" to "GB", "Britain" to "GB", "Англия" to "GB", "Британия" to "GB",
        "Holland" to "NL", "Голландия" to "NL", "Turkey" to "TR", "Emirates" to "AE", "Эмираты" to "AE",
        "Korea" to "KR", "Корея" to "KR", "Czech" to "CZ", "Белоруссия" to "BY",
    )

    /** Города: по ним узнаём страну, но из подписи не убираем — «Amsterdam» и есть имя сервера. */
    private val cities = listOf(
        "NL" to "Amsterdam Амстердам Rotterdam", "DE" to "Frankfurt Франкфурт Berlin Берлин Munich Мюнхен Nuremberg Falkenstein",
        "FI" to "Helsinki Хельсинки", "FR" to "Paris Париж Gravelines Roubaix", "GB" to "London Лондон Manchester",
        "US" to "New_York Нью-Йорк Los_Angeles Лос-Анджелес Miami Майами Chicago Dallas Seattle Ashburn",
        "TR" to "Istanbul Стамбул Ankara", "JP" to "Tokyo Токио Osaka", "SE" to "Stockholm Стокгольм", "PL" to "Warsaw Варшава",
        "KZ" to "Almaty Алматы Astana Астана", "LV" to "Riga Рига", "LT" to "Vilnius Вильнюс", "EE" to "Tallinn Таллин",
        "CH" to "Zurich Цюрих Geneva", "AT" to "Vienna Вена", "CZ" to "Prague Прага", "IT" to "Milan Милан Rome",
        "ES" to "Madrid Мадрид Barcelona", "CA" to "Toronto Торонто Montreal Vancouver", "AE" to "Dubai Дубай",
        "RU" to "Moscow Москва Мск Петербург Petersburg", "BY" to "Minsk Минск", "UA" to "Kyiv Kiev Киев", "GE" to "Tbilisi Тбилиси",
        "AM" to "Yerevan Ереван", "RS" to "Belgrade Белград", "HU" to "Budapest Будапешт", "NO" to "Oslo Осло",
        "IL" to "Tel_Aviv Тель-Авив", "KR" to "Seoul Сеул", "AU" to "Sydney Сидней",
    ).flatMap { (code, list) -> list.split(" ").map { it.replace('_', ' ') to code } }

    private fun norm(s: String) = s.lowercase().replace('ё', 'е')

    private class Pattern(val re: Regex, val code: String, val country: Boolean)

    /** Русское название ловим по основе с падежным окончанием: «Германия», «в Германии» — но не «Индиго». */
    private fun pattern(name: String, code: String, country: Boolean): Pattern {
        var n = norm(name.replace(Regex("\\s*\\(.*\\)$"), ""))
        val stem = Regex("[а-я]").containsMatchIn(n) && n.length > 4
        if (stem && n.last() in "аяыиьй") n = n.dropLast(1)
        val tail = (if (stem) "(?:[аеиоуыьюяй]|ами|ам|ах|ов|ом|ой|ей|ью|ею|ии)?" else "") + "(?!\\p{L})"
        return Pattern(Regex("(?:^|[^\\p{L}])(" + Regex.escape(n) + tail + ")"), code, country)
    }

    private val patterns: List<Pattern> by lazy {
        val seen = HashSet<String>()
        val out = mutableListOf<Triple<String, String, Boolean>>()
        fun add(name: String?, code: String, country: Boolean) {
            if (name.isNullOrEmpty() || name == code || !seen.add(norm(name))) return
            out += Triple(name, code, country)
        }
        for (code in namedCodes) {
            add(Locale("", code).getDisplayCountry(ru), code, true)
            add(Locale("", code).getDisplayCountry(en), code, true)
        }
        for ((n, code) in aliases) add(n, code, true)
        for ((n, code) in cities) add(n, code, false)
        // Длинные раньше коротких: «Южная Корея» прежде «Корея»; страны прежде городов.
        out.sortWith(compareBy<Triple<String, String, Boolean>> { !it.third }.thenByDescending { it.first.length })
        out.map { pattern(it.first, it.second, it.third) }
    }

    private class Found(val code: String, val at: Int, val len: Int, val country: Boolean)

    private fun byName(text: String): Found? {
        val t = norm(text)
        for (p in patterns) {
            val m = p.re.find(t) ?: continue
            val g = m.groups[1] ?: continue
            return Found(p.code, g.range.first, g.value.length, p.country)
        }
        return null
    }

    private val proto = mapOf(
        "hysteria2" to ("HY2" to "Hysteria2"), "hy2" to ("HY2" to "Hysteria2"), "hysteria" to ("HY" to "Hysteria"),
        "vless" to ("VLESS" to "VLESS"), "vmess" to ("VMess" to "VMess"), "trojan" to ("Trojan" to "Trojan"),
        "ss" to ("SS" to "Shadowsocks"), "shadowsocks" to ("SS" to "Shadowsocks"), "ssr" to ("SSR" to "ShadowsocksR"),
        "tuic" to ("TUIC" to "TUIC"), "wireguard" to ("WG" to "WireGuard"), "anytls" to ("AnyTLS" to "AnyTLS"),
        "socks5" to ("SOCKS5" to "SOCKS5"), "http" to ("HTTP" to "HTTP"),
    )

    /** Сокращения протоколов в имени — полными словами: «HY2» → «Hysteria2», «AWG3.1» → «AmneziaWG 3.1». */
    private fun humanize(s0: String): String {
        var s = s0.replace(Regex("\\bHY(?:STERIA)?2\\b", RegexOption.IGNORE_CASE), "Hysteria2")
        s = s.replace(Regex("\\bA(?:MNEZIA)?WG[ -]?(\\d+(?:\\.\\d+)?)\\b", RegexOption.IGNORE_CASE), "AmneziaWG $1")
        s = s.replace(Regex("\\bAWG\\b", RegexOption.IGNORE_CASE), "AmneziaWG")
        s = s.replace(Regex("\\bWG\\b"), "WireGuard")
        s = s.replace(Regex("\\bVLESS\\b", RegexOption.IGNORE_CASE), "VLESS")
        return s.replace(Regex("\\bVMESS\\b", RegexOption.IGNORE_CASE), "VMess")
    }

    private val sep = Regex("^[\\s·•|\\-–—_:,/\\\\]+|[\\s·•|\\-–—_:,/\\\\]+$")
    private fun trim(s: String) = s.replace(sep, "")

    /** Код в начале имени: «BY · …», «DE-2», «[NL] …», «(US) …». */
    private val lead = Regex("^[\\[(]?([A-Z]{2})[\\])]?(?=$|[\\s·•|\\-–—_:,./\\\\\\d])")

    private fun isFlagLetter(cp: Int) = cp in 0x1F1E6..0x1F1FF

    /** Флаг-эмодзи в начале названия: код страны и остаток. */
    fun splitFlag(name: String): Pair<String?, String> {
        val t = name.trimStart()
        val cps = t.codePoints().toArray()
        if (cps.size < 2 || !isFlagLetter(cps[0]) || !isFlagLetter(cps[1])) return null to name
        val code = "${('A' + (cps[0] - 0x1F1E6))}${('A' + (cps[1] - 0x1F1E6))}"
        return code to String(cps, 2, cps.size - 2).trim()
    }

    /** Разбор одного имени — один раз: экран перерисовывается каждую секунду. */
    private val cache = HashMap<String, ParsedServer>()

    fun parse(name: String, kind: String): ParsedServer = cache.getOrPut("$kind\n$name") { parseUncached(name, kind) }

    private fun parseUncached(name: String, kind: String): ParsedServer {
        // Флаг — два символа regional indicator: 🇧🇾 = B + Y.
        val cps = name.codePoints().toArray()
        val flag = cps.filter(::isFlagLetter)
        var code: String? = if (flag.size >= 2) "${('A' + (flag[0] - 0x1F1E6))}${('A' + (flag[1] - 0x1F1E6))}" else null
        val noFlag = cps.filterNot(::isFlagLetter).toIntArray()
        var rest = trim(String(noFlag, 0, noFlag.size))

        lead.find(rest)?.let { m ->
            val c = m.groupValues[1]
            if (isRegion(c) && (code == null || code == c)) {
                code = c
                rest = trim(rest.substring(m.value.length))
            }
        }

        val found = byName(rest)
        if (found != null && (code == null || code == found.code)) {
            code = found.code
            // «Германия · Frankfurt» → «Frankfurt»; город и название в середине имени оставляем.
            if (found.country && found.at == 0) rest = trim(rest.substring(minOf(found.len, rest.length)))
        }

        val k = kind.lowercase().removeSuffix("+realm")
        var (tag, human) = proto[k] ?: (kind.uppercase() to kind)
        if (k == "wireguard" && Regex("\\b(?:AWG|amnezia)", RegexOption.IGNORE_CASE).containsMatchIn(name)) {
            tag = "AWG"
            human = "AmneziaWG"
        }
        val title = humanize(rest)
        return ParsedServer(code, title, tag, title.lowercase().contains(human.lowercase()))
    }

    /** Серверы по странам: `pin` — сверху, дальше по алфавиту, «Другие» — в конце. Внутри страны — порядок подписки. */
    fun group(list: List<Srv>, pin: String?): List<CountryGroup> {
        val map = LinkedHashMap<String, MutableList<Pair<Srv, ParsedServer>>>()
        val codes = HashMap<String, String?>()
        for (s in list) {
            val p = parse(s.name, s.kind)
            val key = p.code ?: ""
            map.getOrPut(key) { mutableListOf() } += s to p
            codes[key] = p.code
        }
        val collator = Collator.getInstance(ru)
        return map.map { (key, items) -> CountryGroup(key, codes[key], name(codes[key]), items) }.sortedWith { a, b ->
            when {
                (a.key == pin) != (b.key == pin) -> if (a.key == pin) -1 else 1
                (a.key == "") != (b.key == "") -> if (b.key == "") -1 else 1
                else -> collator.compare(a.name, b.name)
            }
        }
    }
}

/** Флаг страны в круге (эмодзи системы), без страны — значок глобуса. */
@Composable
fun FlagView(code: String?, size: Int = 28) {
    Box(Modifier.size(size.dp).clip(CircleShape).background(K.elem), contentAlignment = Alignment.Center) {
        if (code != null) KText(Countries.emoji(code), size = (size * 0.78).sp) else KIcon(KIcons.globe, (size * 0.54).dp, K.dim)
    }
}

/** Раскрытые страны живут, пока открыто приложение. */
private var remembered: Set<String>? = null

/**
 * Серверы по странам — как ui/src/components/ServerCountries.tsx: строка страны (флаг, название, сколько
 * серверов, протоколы, лучшая задержка), по нажатию — её серверы. Порядок стран от задержек не зависит.
 */
@Composable
fun ServerCountries(list: List<Srv>, current: Srv?, pinging: Boolean, tested: Boolean, onPick: (String) -> Unit) {
    val haptics = LocalHapticFeedback.current
    val cur = current?.let { Countries.parse(it.name, it.kind) }
    val curKey = cur?.let { it.code ?: "" }
    var open by remember { mutableStateOf(remembered ?: setOfNotNull(curKey)) }
    // Страна выбранного сервера — сверху, но только на момент открытия: выбор в другой стране строки не двигает.
    val pin = remember { curKey }
    Column(verticalArrangement = Arrangement.spacedBy(2.dp)) {
        Countries.group(list, pin).forEach { g ->
            val isOpen = g.key in open
            val isCur = g.key == curKey
            val best = g.items.mapNotNull { it.first.delay }.minOrNull()
            val turn by animateFloatAsState(if (isOpen) 90f else 0f, label = "chevron")
            Column {
                Row(
                    Modifier.fillMaxWidth().clip(RoundedCornerShape(K.r(10))).clickable {
                        haptics.performHapticFeedback(HapticFeedbackType.TextHandleMove)
                        open = if (isOpen) open - g.key else open + g.key
                        remembered = open
                    }.padding(horizontal = 8.dp, vertical = 9.dp),
                    verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(12.dp),
                ) {
                    Box(Modifier.border(1.5.dp, if (isCur) K.accent else Color.Transparent, CircleShape).padding(3.5.dp)) { FlagView(g.code) }
                    Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(3.dp)) {
                        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                            KText(g.name, weight = FontWeight.SemiBold, lines = 1)
                            KText("${g.items.size}", size = 12.sp, color = K.dim, mono = true)
                        }
                        Row(Modifier.height(16.dp), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(4.dp)) {
                            if (isCur && current != null && cur != null) {
                                Box(Modifier.padding(end = 2.dp).size(6.dp).background(K.accent, CircleShape))
                                KText(title(current, cur), size = 12.sp, color = K.text2, lines = 1)
                            } else {
                                g.tags.forEach { Tag(it) }
                            }
                        }
                    }
                    Ping(best, pinging, tested, 13)
                    KIcon(KIcons.chevron, 14.dp, K.dim2, Modifier.rotate(turn))
                }
                AnimatedVisibility(isOpen, enter = fadeIn(), exit = fadeOut()) {
                    Row(Modifier.padding(start = 21.dp, bottom = 6.dp)) {
                        Box(Modifier.width(1.5.dp).height((g.items.size * 40).dp).background(K.line))
                        Column(Modifier.padding(start = 9.dp), verticalArrangement = Arrangement.spacedBy(2.dp)) {
                            g.items.forEach { (s, p) -> CountryRow(s, p, s.name == current?.name, pinging, tested, onPick) }
                        }
                    }
                }
            }
        }
    }
}

@Composable
private fun CountryRow(s: Srv, p: ParsedServer, on: Boolean, pinging: Boolean, tested: Boolean, onPick: (String) -> Unit) {
    val haptics = LocalHapticFeedback.current
    val mute = tested && !pinging && s.delay == null
    val proto = if (p.title.isNotEmpty() && !p.named) Texts.protocol(s.kind) else null
    Row(
        Modifier.fillMaxWidth().height(38.dp).clip(RoundedCornerShape(K.r(10))).background(if (on) K.hover else Color.Transparent)
            .clickable {
                haptics.performHapticFeedback(HapticFeedbackType.TextHandleMove)
                if (!on) onPick(s.name)
            }.padding(horizontal = 8.dp),
        verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(10.dp),
    ) {
        Box(Modifier.size(18.dp).border(2.dp, if (on) K.accent else K.ring, CircleShape), contentAlignment = Alignment.Center) {
            if (on) Box(Modifier.size(8.dp).background(K.accent, CircleShape))
        }
        KText(title(s, p), size = 13.5.sp, weight = if (on) FontWeight.SemiBold else FontWeight.Medium, color = if (mute) K.dim else K.text, lines = 1, modifier = Modifier.weight(1f))
        if (proto != null) Tag(proto)
        s.speed?.let { KText(Texts.mbps(it), size = 11.sp, color = K.dim, mono = true) }
        Ping(s.delay, pinging, tested, 12.5)
    }
}

private fun title(s: Srv, p: ParsedServer) = p.title.ifEmpty { Texts.protocol(s.kind) }

/** `.cg-tag`: метка протокола — 16, скругление 5, 10 полужирный. */
@Composable
private fun Tag(text: String) {
    Box(Modifier.height(16.dp).clip(RoundedCornerShape(5.dp)).background(K.elem).padding(horizontal = 5.dp), contentAlignment = Alignment.Center) {
        KText(text, size = 10.sp, weight = FontWeight.SemiBold, tracking = 0.3.sp, color = K.text3)
    }
}

/** Задержка: «…» во время замера, пусто до первого замера — null тогда не значит «нет ответа». */
@Composable
private fun Ping(delay: Int?, pinging: Boolean, tested: Boolean, size: Number) {
    val sp = size.toFloat().sp
    when {
        pinging -> KText("…", size = sp, color = K.dim, modifier = Modifier.widthIn(min = 44.dp))
        delay != null -> KText("$delay мс", size = sp, mono = true, color = K.ping(delay), modifier = Modifier.widthIn(min = 44.dp))
        tested -> Box(Modifier.height(20.dp).clip(CircleShape).background(K.red.copy(alpha = 0.13f)).padding(horizontal = 8.dp), contentAlignment = Alignment.Center) {
            KText("нет ответа", size = 11.sp, weight = FontWeight.SemiBold, color = K.red.copy(alpha = 0.9f))
        }
        else -> Spacer(Modifier.width(44.dp))
    }
}
