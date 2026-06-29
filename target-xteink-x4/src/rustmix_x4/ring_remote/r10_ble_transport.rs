// COLMI R10 BLE transport contract.
//
// This is not the hardware BLE implementation yet. It captures the UUIDs,
// command packets, polling cadence, and notify-to-action bridge so the later
// esp-radio + BLE-host task has a small, tested boundary.

use super::r10_input_bridge::try_enqueue_remote_action;
use super::r10_protocol::{R10_REMOTE_POLL, R10_REMOTE_START, R10_REMOTE_STOP};
use super::r10_remote_policy::{R10RemoteAction, R10RemotePolicy};

pub const R10_BLE_SERVICE_UUID: &str = "6e40fff0-b5a3-f393-e0a9-e50e24dcca9e";
pub const R10_BLE_WRITE_UUID: &str = "6e400002-b5a3-f393-e0a9-e50e24dcca9e";
pub const R10_BLE_NOTIFY_UUID: &str = "6e400003-b5a3-f393-e0a9-e50e24dcca9e";

pub const R10_BLE_POLL_INTERVAL_MS: u64 = 1_000;
pub const R10_BLE_RECONNECT_BACKOFF_MS: u64 = 3_000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R10BleCommand {
    StartRemote,
    PollRemote,
    StopRemote,
}

impl R10BleCommand {
    pub fn packet(self) -> &'static [u8; 16] {
        match self {
            Self::StartRemote => R10_REMOTE_START.as_slice(),
            Self::PollRemote => R10_REMOTE_POLL.as_slice(),
            Self::StopRemote => R10_REMOTE_STOP.as_slice(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R10BleTransportState {
    Disabled,
    Idle,
    Scanning,
    Connecting,
    Discovering,
    Subscribing,
    StartingRemote,
    Polling,
    Disconnecting,
    Backoff,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R10BleRemoteSession {
    pub state: R10BleTransportState,
    pub policy: R10RemotePolicy,
    pub poll_interval_ms: u64,
    pub next_poll_due_ms: u64,
}

impl R10BleRemoteSession {
    pub const fn disabled() -> Self {
        Self {
            state: R10BleTransportState::Disabled,
            policy: R10RemotePolicy::disabled(),
            poll_interval_ms: R10_BLE_POLL_INTERVAL_MS,
            next_poll_due_ms: 0,
        }
    }

    pub const fn reader_remote(debounce_ms: u64) -> Self {
        Self {
            state: R10BleTransportState::Idle,
            policy: R10RemotePolicy::enabled_for_reader(debounce_ms),
            poll_interval_ms: R10_BLE_POLL_INTERVAL_MS,
            next_poll_due_ms: 0,
        }
    }

    pub fn on_notify(&mut self, now_ms: u64, packet: &[u8]) -> R10RemoteAction {
        self.policy.handle_packet(now_ms, packet)
    }

    pub fn on_notify_enqueue(&mut self, now_ms: u64, packet: &[u8]) -> bool {
        let action = self.on_notify(now_ms, packet);
        try_enqueue_remote_action(action)
    }

    pub fn should_poll(&self, now_ms: u64) -> bool {
        matches!(self.state, R10BleTransportState::Polling) && now_ms >= self.next_poll_due_ms
    }

    pub fn mark_poll_sent(&mut self, now_ms: u64) {
        self.next_poll_due_ms = now_ms.saturating_add(self.poll_interval_ms);
    }

    pub fn start_polling(&mut self, now_ms: u64) {
        self.state = R10BleTransportState::Polling;
        self.next_poll_due_ms = now_ms;
    }

    pub fn enter_backoff(&mut self, now_ms: u64) {
        self.state = R10BleTransportState::Backoff;
        self.next_poll_due_ms = now_ms.saturating_add(R10_BLE_RECONNECT_BACKOFF_MS);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rustmix_x4::contracts::input_semantics::RustmixReaderAction;
    use crate::rustmix_x4::ring_remote::r10_remote_policy::R10RemoteAction;

    const MOTION: [u8; 16] = [0x02, 0x02, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x04];

    #[test]
    fn r10_ble_uuid_contract_matches_stock_gatt() {
        assert_eq!(R10_BLE_SERVICE_UUID, "6e40fff0-b5a3-f393-e0a9-e50e24dcca9e");
        assert_eq!(R10_BLE_WRITE_UUID, "6e400002-b5a3-f393-e0a9-e50e24dcca9e");
        assert_eq!(R10_BLE_NOTIFY_UUID, "6e400003-b5a3-f393-e0a9-e50e24dcca9e");
    }

    #[test]
    fn r10_ble_commands_use_known_stock_packets() {
        assert_eq!(
            R10BleCommand::StartRemote.packet(),
            &[0x02, 0x04, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x06]
        );
        assert_eq!(
            R10BleCommand::PollRemote.packet(),
            &[0x02, 0x05, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x07]
        );
        assert_eq!(
            R10BleCommand::StopRemote.packet(),
            &[0x02, 0x06, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x08]
        );
    }

    #[test]
    fn r10_ble_notify_uses_debounced_policy() {
        let mut session = R10BleRemoteSession::reader_remote(3500);
        assert_eq!(
            session.on_notify(10_000, &MOTION),
            R10RemoteAction::Reader(RustmixReaderAction::NextPage)
        );
        assert_eq!(session.on_notify(11_000, &MOTION), R10RemoteAction::None);
    }

    #[test]
    fn r10_ble_poll_timer_is_explicit() {
        let mut session = R10BleRemoteSession::reader_remote(3500);
        assert!(!session.should_poll(10_000));

        session.start_polling(10_000);
        assert!(session.should_poll(10_000));

        session.mark_poll_sent(10_000);
        assert!(!session.should_poll(10_999));
        assert!(session.should_poll(11_000));
    }
}
