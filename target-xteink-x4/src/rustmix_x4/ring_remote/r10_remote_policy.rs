// Transport-independent R10 remote policy.
//
// This module decides whether an R10 motion packet should become a Reader
// action. It deliberately does not know about BLE, display, SD, or app state.

use super::r10_protocol::{R10RemotePacket, R10RemotePacketKind};
use crate::rustmix_x4::contracts::input_semantics::RustmixReaderAction;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R10RemoteScreen {
    Reader,
    Home,
    Library,
    Settings,
    WifiTransfer,
    Sleep,
    Confirm,
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R10RemoteAction {
    None,
    Reader(RustmixReaderAction),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R10RemoteDebounce {
    last_accept_ms: Option<u64>,
    debounce_ms: u64,
}

impl R10RemoteDebounce {
    pub const fn new(debounce_ms: u64) -> Self {
        Self {
            last_accept_ms: None,
            debounce_ms,
        }
    }

    pub fn reset(&mut self) {
        self.last_accept_ms = None;
    }

    pub fn accept(&mut self, now_ms: u64) -> bool {
        match self.last_accept_ms {
            Some(last) if now_ms.saturating_sub(last) < self.debounce_ms => false,
            _ => {
                self.last_accept_ms = Some(now_ms);
                true
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R10RemotePolicy {
    pub enabled: bool,
    pub screen: R10RemoteScreen,
    pub debounce: R10RemoteDebounce,
}

impl R10RemotePolicy {
    pub const fn disabled() -> Self {
        Self {
            enabled: false,
            screen: R10RemoteScreen::Unknown,
            debounce: R10RemoteDebounce::new(3500),
        }
    }

    pub const fn enabled_for_reader(debounce_ms: u64) -> Self {
        Self {
            enabled: true,
            screen: R10RemoteScreen::Reader,
            debounce: R10RemoteDebounce::new(debounce_ms),
        }
    }

    pub const fn screen_allows_reader_action(screen: R10RemoteScreen) -> bool {
        matches!(screen, R10RemoteScreen::Reader)
    }

    pub fn handle_packet(&mut self, now_ms: u64, packet: &[u8]) -> R10RemoteAction {
        if !self.enabled || !Self::screen_allows_reader_action(self.screen) {
            return R10RemoteAction::None;
        }

        match R10RemotePacket::classify(packet) {
            R10RemotePacketKind::RemoteMotion if self.debounce.accept(now_ms) => {
                R10RemoteAction::Reader(RustmixReaderAction::NextPage)
            }
            _ => R10RemoteAction::None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MOTION: [u8; 16] = [0x02, 0x02, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x04];

    #[test]
    fn r10_motion_maps_to_reader_next_page() {
        let mut policy = R10RemotePolicy::enabled_for_reader(3500);
        assert_eq!(
            policy.handle_packet(10_000, &MOTION),
            R10RemoteAction::Reader(RustmixReaderAction::NextPage)
        );
    }

    #[test]
    fn r10_motion_is_reader_only() {
        let mut policy = R10RemotePolicy {
            enabled: true,
            screen: R10RemoteScreen::Settings,
            debounce: R10RemoteDebounce::new(3500),
        };
        assert_eq!(policy.handle_packet(10_000, &MOTION), R10RemoteAction::None);
    }

    #[test]
    fn r10_motion_is_debounced() {
        let mut policy = R10RemotePolicy::enabled_for_reader(3500);
        assert_eq!(
            policy.handle_packet(10_000, &MOTION),
            R10RemoteAction::Reader(RustmixReaderAction::NextPage)
        );
        assert_eq!(policy.handle_packet(11_000, &MOTION), R10RemoteAction::None);
        assert_eq!(
            policy.handle_packet(14_000, &MOTION),
            R10RemoteAction::Reader(RustmixReaderAction::NextPage)
        );
    }

    #[test]
    fn r10_disabled_ignores_motion() {
        let mut policy = R10RemotePolicy::disabled();
        policy.screen = R10RemoteScreen::Reader;
        assert_eq!(policy.handle_packet(10_000, &MOTION), R10RemoteAction::None);
    }
}
