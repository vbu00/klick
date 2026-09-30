//! Под какую систему работает kl!ck. Пути программ и конфиг ядра на Windows и macOS устроены
//! по-разному, но логика обеих систем — чистые функции: тесты проверяют обе на любой машине.

/// Система, для которой собираются правила и конфиг ядра.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Os {
    Windows,
    /// macOS; так же ведут себя и другие Unix-системы: пути через `/`.
    MacOs,
}

impl Os {
    /// Система, под которую собрана программа.
    pub const CURRENT: Os = if cfg!(windows) { Os::Windows } else { Os::MacOs };

    pub fn separator(self) -> char {
        match self {
            Os::Windows => '\\',
            Os::MacOs => '/',
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
