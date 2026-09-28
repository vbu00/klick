//! Последние записи журнала в памяти — для экрана «Журнал» и «Скопировать отчёт».
//! Файл журнала закрыт от обычных пользователей, поэтому окно получает записи по каналу.

use klick_proto::LogLine;
use std::collections::VecDeque;
use std::fmt::Write as _;
use std::sync::Mutex;
use tracing::field::{Field, Visit};
use tracing::{Event, Level, Subscriber};
use tracing_subscriber::layer::{Context, Layer};

const KEPT: usize = 400;

static RING: Mutex<VecDeque<LogLine>> = Mutex::new(VecDeque::new());

/// Слой подписчика tracing: каждое событие — строкой в кольцо.
pub struct RingLayer;

impl<S: Subscriber> Layer<S> for RingLayer {
    fn on_event(&self, event: &Event<'_>, _ctx: Context<'_, S>) {
        let mut text = Message::default();
        event.record(&mut text);
        let level = match *event.metadata().level() {
            Level::ERROR => "error",
            Level::WARN => "warn",
            Level::INFO => "info",
            Level::DEBUG => "debug",
            Level::TRACE => "trace",
        };
        let at = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0);
        let mut ring = RING.lock().unwrap_or_else(|e| e.into_inner());
        if ring.len() >= KEPT {
            ring.pop_front();
        }
        ring.push_back(LogLine { at, level: level.into(), text: text.0 });
    }
}

#[derive(Default)]
struct Message(String);

impl Visit for Message {
    fn record_str(&mut self, field: &Field, value: &str) {
        if field.name() == "message" {
            self.0.insert_str(0, value);
        } else {
            let _ = write!(self.0, " {}={value}", field.name());
        }
    }

    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        if field.name() == "message" {
            self.0.insert_str(0, &format!("{value:?}"));
        } else {
            let _ = write!(self.0, " {}={value:?}", field.name());
        }
    }
}

pub fn recent() -> Vec<LogLine> {
    RING.lock().unwrap_or_else(|e| e.into_inner()).iter().cloned().collect()
}

pub fn clear() {
    RING.lock().unwrap_or_else(|e| e.into_inner()).clear();
}
