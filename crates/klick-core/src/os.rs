//! Под какую систему работает kl!ck. Пути программ и конфиг ядра на Windows, macOS и iOS устроены
//! по-разному, но логика всех систем — чистые функции: тесты проверяют их на любой машине.

/// Система, для которой собираются правила и конфиг ядра.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Os {
    Windows,
    /// macOS; так же ведут себя и другие Unix-системы: пути через `/`.
    MacOs,
    /// iOS: адаптер держит расширение VPN, а не ядро, и программ в правилах нет — система не говорит,
    /// чьё соединение.
    Ios,
    /// Android: как iOS — адаптер держит VpnService приложения, а не ядро, и ядро не знает, чьё соединение
    /// (без прав root `/proc/net` закрыт).
    Android,
}

impl Os {
    /// Система, под которую собрана программа.
    pub const CURRENT: Os = if cfg!(windows) {
        Os::Windows
    } else if cfg!(target_os = "ios") {
        Os::Ios
    } else if cfg!(target_os = "android") {
        Os::Android
    } else {
        Os::MacOs
    };

    /// Телефон: туннель держит система (расширение iOS, VpnService Android), в конфиге нет раздела `tun`.
    pub fn mobile(self) -> bool {
        matches!(self, Os::Ios | Os::Android)
    }

    pub fn separator(self) -> char {
        match self {
            Os::Windows => '\\',
            Os::MacOs | Os::Ios | Os::Android => '/',
        }
    }

    /// Сколько уровней в пути папки: чем глубже папка, тем точнее правило.
    pub fn depth(self, folder: &str) -> usize {
        let sep = self.separator();
        folder.trim_end_matches(sep).split(sep).filter(|s| !s.is_empty()).count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn depth_counts_segments() {
        assert_eq!(Os::Windows.depth(r"C:\Games\Roblox\"), 3);
        assert_eq!(Os::MacOs.depth("/Applications/Discord.app/"), 2);
        assert_eq!(Os::MacOs.depth("/Users/a/Library/Application Support/x"), 5);
    }
}
