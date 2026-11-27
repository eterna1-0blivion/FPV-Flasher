//! Протокол Walksnail Ascent (AR). Описание — в `docs/protocols/walksnail-ar.md`.

pub mod device;
pub mod frame;
pub mod image;
pub mod upgrade;

pub const STATUS: &str = "Обновление Walksnail пока недоступно: раздел в разработке.";

pub use device::{DeviceInfo, Side};
pub use frame::{ArCommand, Command, Frame, FrameError};
