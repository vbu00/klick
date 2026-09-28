//! Страж связи: когда проверять сервер, когда переподключаться и когда сдаться.
//!
//! Время — миллисекунды от любой точки отсчёта, поэтому логика проверяется без часов и сна.
//! По умолчанию: проверка раз в 30 с; после неудачи попытки через 2, 5 и 8 с;
//! на 15-й секунде уведомление «Сервер не отвечает»; дальше тихая проверка раз в минуту.

pub const PROBE_EVERY_MS: u64 = 30_000;
pub const RETRY_AT_MS: [u64; 3] = [2_000, 5_000, 8_000];
pub const GIVE_UP_AT_MS: u64 = 15_000;
pub const QUIET_EVERY_MS: u64 = 60_000;
pub const ATTEMPTS: u8 = RETRY_AT_MS.len() as u8;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Health {
    Healthy,
    /// `done` — сколько попыток уже сделано и провалено.
    Retrying { done: u8 },
    Down,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    /// Обычная проверка связи через сервер.
    Probe,
    /// Попытка переподключения `attempt` из `of`.
    Retry { attempt: u8, of: u8 },
    /// Попытки кончились: сообщить «Сервер не отвечает».
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

    /// Какую по счёту попытку показывать в интерфейсе: «Переподключаюсь… 2 из 3».
    pub fn attempt_shown(&self) -> Option<(u8, u8)> {
        match self.health {
            Health::Retrying { done } => Some(((done + 1).min(ATTEMPTS), ATTEMPTS)),
            _ => None,
        }
    }

    /// Когда и что делать дальше.
    pub fn next(&self) -> (u64, Action) {
        match self.health {
            Health::Healthy => (self.last_check + PROBE_EVERY_MS, Action::Probe),
            Health::Retrying { done } => match RETRY_AT_MS.get(done as usize) {
                Some(at) => (self.failed_at + at, Action::Retry { attempt: done + 1, of: ATTEMPTS }),
                None => (self.failed_at + GIVE_UP_AT_MS, Action::GiveUp),
            },
            Health::Down => (self.last_check + QUIET_EVERY_MS, Action::Probe),
        }
    }

    /// Итог проверки или попытки.
    pub fn report(&mut self, ok: bool, now: u64) -> Option<Notice> {
        self.last_check = now;
        match (self.health, ok) {
            (Health::Healthy, true) => None,
            (Health::Healthy, false) => {
                self.health = Health::Retrying { done: 0 };
                self.failed_at = now;
                None
            }
            (Health::Retrying { .. }, true) => {
                self.health = Health::Healthy;
                None
            }
            (Health::Retrying { done }, false) => {
                self.health = Health::Retrying { done: (done + 1).min(ATTEMPTS) };
                None
            }
            (Health::Down, true) => {
                self.health = Health::Healthy;
                Some(Notice::Restored)
            }
            (Health::Down, false) => None,
        }
    }

    /// Время вышло: переходим к тихой проверке раз в минуту.
    pub fn give_up(&mut self, now: u64) -> Notice {
        self.health = Health::Down;
        self.last_check = now;
        Notice::Down
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
    fn failure_walks_through_2_5_8_then_gives_up_at_15() {
        let mut g = Guard::new(0);
        assert_eq!(g.report(false, 30_000), None);
        assert_eq!(g.next(), (32_000, Action::Retry { attempt: 1, of: 3 }));
        assert_eq!(g.attempt_shown(), Some((1, 3)));
        g.report(false, 32_500);
        assert_eq!(g.next(), (35_000, Action::Retry { attempt: 2, of: 3 }));
        g.report(false, 35_500);
        assert_eq!(g.next(), (38_000, Action::Retry { attempt: 3, of: 3 }));
        g.report(false, 38_500);
        assert_eq!(g.next(), (45_000, Action::GiveUp));
        assert_eq!(g.attempt_shown(), Some((3, 3)));
        assert_eq!(g.give_up(45_000), Notice::Down);
        assert_eq!(g.health(), Health::Down);
        assert_eq!(g.next(), (105_000, Action::Probe));
    }

    #[test]
    fn quick_recovery_is_silent() {
        let mut g = Guard::new(0);
        g.report(false, 30_000);
        assert_eq!(g.report(true, 32_100), None);
        assert_eq!(g.health(), Health::Healthy);
        assert_eq!(g.next(), (62_100, Action::Probe));
    }

    #[test]
    fn recovery_after_give_up_is_announced() {
        let mut g = Guard::new(0);
        g.report(false, 0);
        g.give_up(15_000);
        assert_eq!(g.report(false, 75_000), None);
        assert_eq!(g.report(true, 135_000), Some(Notice::Restored));
        assert_eq!(g.health(), Health::Healthy);
    }
}
