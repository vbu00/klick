//! Мозг kl!ck: всё, что не зависит от операционной системы.
//!
//! Здесь живут модель настроек, сборка конфигурации mihomo, политика стража связи
//! и разбор заголовков подписок. Модуль не делает системных вызовов и не ходит в сеть,
//! поэтому целиком покрывается обычными тестами и переносится на другие ОС.

pub mod compile;
pub mod convert;
pub mod deeplink;
pub mod guard;
pub mod macos;
pub mod model;
pub mod os;
pub mod sub;

pub use model::*;
pub use os::Os;
