//! Итог локальной проверки сервера.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tcp {
    Open,
    Refused,
    Timeout,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dl {
    Complete,
    Stalled(u64),
    Failed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProbeVerdict {
    Ok,
    Freeze,
    TcpBlocked,
    HandshakeFailed,
    NoAnswer,
}

/// Сводит несколько попыток к одному результату: одиночный сбой не помечает сервер негодным.
pub fn verdict(tcp: Option<Tcp>, delay_ok: bool, dls: &[Dl]) -> (ProbeVerdict, bool) {
    let frozen = !dls.is_empty()
        && dls.iter().all(|dl| matches!(dl, Dl::Stalled(bytes) if (1..65_536).contains(bytes)))
        && (delay_ok || tcp == Some(Tcp::Open));
    if frozen {
        return (ProbeVerdict::Freeze, false);
    }

    if dls.contains(&Dl::Complete) || delay_ok {
        let flaky = !delay_ok || dls.iter().any(|dl| *dl != Dl::Complete) || tcp.is_some_and(|state| state != Tcp::Open);
        return (ProbeVerdict::Ok, flaky);
    }

    let result = match tcp {
        Some(Tcp::Timeout) => ProbeVerdict::TcpBlocked,
        Some(Tcp::Open) => ProbeVerdict::HandshakeFailed,
        Some(Tcp::Refused) | None => ProbeVerdict::NoAnswer,
    };
    (result, false)
}

#[cfg(test)]
mod tests {
    use super::{verdict, Dl, ProbeVerdict, Tcp};

    #[test]
    fn verdict_uses_measured_cases() {
        let cases = [
            (Some(Tcp::Open), true, &[Dl::Stalled(10_985), Dl::Stalled(10_985)][..], ProbeVerdict::Freeze, false),
            (None, false, &[Dl::Failed][..], ProbeVerdict::NoAnswer, false),
            (Some(Tcp::Timeout), false, &[Dl::Failed, Dl::Failed][..], ProbeVerdict::TcpBlocked, false),
            (Some(Tcp::Open), false, &[Dl::Failed, Dl::Failed][..], ProbeVerdict::HandshakeFailed, false),
            (None, false, &[Dl::Complete][..], ProbeVerdict::Ok, true),
            (None, true, &[Dl::Failed, Dl::Complete][..], ProbeVerdict::Ok, true),
            (Some(Tcp::Open), true, &[Dl::Complete, Dl::Complete][..], ProbeVerdict::Ok, false),
            (None, true, &[Dl::Stalled(15_062)][..], ProbeVerdict::Freeze, false),
            (Some(Tcp::Open), false, &[Dl::Stalled(65_536)][..], ProbeVerdict::HandshakeFailed, false),
        ];
        for (tcp, delay_ok, dls, expected, flaky) in cases {
            assert_eq!(verdict(tcp, delay_ok, dls), (expected, flaky), "{tcp:?} {delay_ok} {dls:?}");
        }
    }
}
