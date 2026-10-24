//! Страж связи: когда проверять сервер, когда перечитывать конфиг и когда сдаться.
//!
//! Время — миллисекунды от любой точки отсчёта, поэтому логика проверяется без часов и сна.
//! Проверка раз в 30 с. Одна неудача — ещё не тревога: проверка через сервер иногда не проходит и у
//! живого сервера (на AmneziaWG — до трети проверок), а перечитанный конфиг рвёт все соединения через
//! VPN. Поэтому после неудачи страж молча проверяет каждые 5 с, считая от конца прошлой проверки;
//! связи нет 20 с подряд — перечитать конфиг (один раз); 45 с — «Сервер не отвечает» и тихая проверка
//! раз в 15 с (вернулся сервер — «Связь восстановлена» почти сразу). Ядро страж не перезапускает:
//! TUN пересоздался бы и оборвал и прямые соединения.

pub const PROBE_EVERY_MS: u64 = 30_000;
pub const RETRY_EVERY_MS: u64 = 5_000;
pub const RELOAD_AFTER_MS: u64 = 20_000;
pub const GIVE_UP_AFTER_MS: u64 = 45_000;
pub const QUIET_EVERY_MS: u64 = 15_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Health {
    Healthy,
    /// Проверки не проходят; `reloaded` — конфиг уже перечитан, и пользователь видит «Переподключаюсь».
    Retrying { reloaded: bool },
    Down,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    /// Обычная проверка связи через сервер.
    Probe,
    /// Повторная проверка после неудачи.
    Retry,
    /// Связи нет давно: перечитать конфиг (новые соединения к серверу) или сменить сервер.
    Reload,
    /// Связи так и нет: сообщить «Сервер не отвечает».
    GiveUp,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Notice {
    Down,
    Restored,
}

#[derive(Clone, Debug)]
pub struct Guard {
    health: Health,
    last_check: u64,
    failed_at: u64,
}

impl Guard {
    pub fn new(now: u64) -> Self {
        Self { health: Health::Healthy, last_check: now, failed_at: now }
    }

    pub fn health(&self) -> Health {
        self.health
    }

    /// Когда и что делать дальше.
    pub fn next(&self) -> (u64, Action) {
        let failing = self.last_check.saturating_sub(self.failed_at);
        match self.health {
            Health::Healthy => (self.last_check + PROBE_EVERY_MS, Action::Probe),
            Health::Retrying { reloaded: false } if failing >= RELOAD_AFTER_MS => (self.last_check, Action::Reload),
            Health::Retrying { reloaded: true } if failing >= GIVE_UP_AFTER_MS => (self.last_check, Action::GiveUp),
            Health::Retrying { .. } => (self.last_check + RETRY_EVERY_MS, Action::Retry),
            Health::Down => (self.last_check + QUIET_EVERY_MS, Action::Probe),
        }
    }

    /// Итог проверки.
    pub fn report(&mut self, ok: bool, now: u64) -> Option<Notice> {
        self.last_check = now;
        match (self.health, ok) {
            (Health::Healthy, false) => {
                self.health = Health::Retrying { reloaded: false };
                self.failed_at = now;
                None
            }
            (Health::Retrying { .. }, true) => {
                self.health = Health::Healthy;
                None
            }
            (Health::Down, true) => {
                self.health = Health::Healthy;
                Some(Notice::Restored)
            }
            _ => None,
        }
    }

    /// Конфиг перечитан: с этого момента пользователь видит «Переподключаюсь».
    pub fn reloading(&mut self) {
        if let Health::Retrying { reloaded } = &mut self.health {
            *reloaded = true;
        }
    }

    /// Время вышло: переходим к тихой проверке раз в 15 с.
    pub fn give_up(&mut self, now: u64) -> Notice {
        self.health = Health::Down;
        self.last_check = now;
        Notice::Down
    }

    /// Сети нет (сон, выключенный Wi-Fi): сервер ни при чём. Ничего не считаем, ждём.
    pub fn wait(&mut self, now: u64) {
        self.last_check = now;
        self.failed_at = now;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn healthy_probes_every_30_seconds() {
        let g = Guard::new(1_000);
        assert_eq!(g.next(), (31_000, Action::Probe));
    }

    #[test]
    fn quick_recovery_is_silent() {
        let mut g = Guard::new(0);
        assert_eq!(g.report(false, 30_000), None);
        assert_eq!(g.health(), Health::Retrying { reloaded: false });
        assert_eq!(g.next(), (35_000, Action::Retry));
        assert_eq!(g.report(true, 35_100), None);
        assert_eq!(g.health(), Health::Healthy);
        assert_eq!(g.next(), (65_100, Action::Probe));
    }

    #[test]
    fn retries_count_from_the_end_of_a_slow_check() {
        let mut g = Guard::new(0);
        g.report(false, 40_000);
        // Повторная проверка шла 10 с: следующая — через 5 с после её конца, а не сразу.
        g.report(false, 55_000);
        assert_eq!(g.next(), (60_000, Action::Retry));
    }

    #[test]
    fn reload_after_20_seconds_then_give_up_after_45() {
        let mut g = Guard::new(0);
        g.report(false, 0);
        for t in [5_000, 10_000, 15_000] {
            assert_eq!(g.next(), (t, Action::Retry));
            g.report(false, t);
        }
        g.report(false, 20_000);
        assert_eq!(g.next(), (20_000, Action::Reload));
        g.reloading();
        assert_eq!(g.health(), Health::Retrying { reloaded: true });
        g.report(false, 21_000);
        assert_eq!(g.next(), (26_000, Action::Retry), "перечитываем один раз");
        g.report(false, 44_000);
        assert_eq!(g.next(), (49_000, Action::Retry));
        g.report(false, 45_000);
        assert_eq!(g.next(), (45_000, Action::GiveUp));
        assert_eq!(g.give_up(45_000), Notice::Down);
        assert_eq!(g.next(), (60_000, Action::Probe));
    }

    #[test]
    fn recovery_after_give_up_is_announced() {
        let mut g = Guard::new(0);
        g.report(false, 0);
        g.give_up(45_000);
        assert_eq!(g.report(false, 105_000), None);
        assert_eq!(g.report(true, 165_000), Some(Notice::Restored));
        assert_eq!(g.health(), Health::Healthy);
    }

    #[test]
    fn no_network_restarts_the_count() {
        let mut g = Guard::new(0);
        g.report(false, 0);
        g.report(false, 15_000);
        g.wait(19_000);
        assert_eq!(g.next(), (24_000, Action::Retry));
        g.report(false, 30_000);
        assert_eq!(g.next(), (35_000, Action::Retry), "20 с считаются заново после возврата сети");
    }
}
