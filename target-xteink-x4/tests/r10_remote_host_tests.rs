#![allow(dead_code)]

mod rustmix_x4 {
    pub mod contracts {
        pub mod input_semantics {
            #[derive(Clone, Copy, Debug, Eq, PartialEq)]
            pub enum RustmixReaderAction {
                BackToLibrary,
                OpenOrSelect,
                NextPage,
                PreviousPage,
                BookmarkOrMenu,
                Unsupported,
            }
        }
    }

    pub mod x4_kernel {
        pub mod board {
            pub mod button {
                #[derive(Clone, Copy, Debug, Eq, PartialEq)]
                pub enum Button {
                    VolDown,
                    VolUp,
                    Right,
                    Left,
                    Confirm,
                    Back,
                    Power,
                }
            }
        }

        pub mod drivers {
            pub mod input {
                use crate::rustmix_x4::x4_kernel::board::button::Button;

                #[derive(Clone, Copy, Debug, Eq, PartialEq)]
                pub enum Event {
                    Press(Button),
                    Release(Button),
                    LongPress(Button),
                    Repeat(Button),
                }
            }
        }

        pub mod kernel {
            pub mod tasks {
                use crate::rustmix_x4::x4_kernel::drivers::input::Event;

                pub struct DummyInputEvents;

                pub static INPUT_EVENTS: DummyInputEvents = DummyInputEvents;

                impl DummyInputEvents {
                    pub fn try_send(&self, _event: Event) -> Result<(), ()> {
                        Ok(())
                    }
                }
            }
        }
    }

    pub mod ring_remote {
        pub mod r10_protocol {
            include!("../src/rustmix_x4/ring_remote/r10_protocol.rs");
        }

        pub mod r10_remote_policy {
            include!("../src/rustmix_x4/ring_remote/r10_remote_policy.rs");
        }

        pub mod r10_input_bridge {
            include!("../src/rustmix_x4/ring_remote/r10_input_bridge.rs");
        }
    }
}

use rustmix_x4::contracts::input_semantics::RustmixReaderAction;
use rustmix_x4::ring_remote::r10_protocol::{
    R10_REMOTE_POLL, R10_REMOTE_START, R10_REMOTE_STOP, R10RemotePacket, R10RemotePacketKind,
};

use rustmix_x4::ring_remote::r10_input_bridge::R10InputInjection;
use rustmix_x4::ring_remote::r10_remote_policy::{
    R10RemoteAction, R10RemotePolicy, R10RemoteScreen,
};
use rustmix_x4::x4_kernel::board::button::Button;
use rustmix_x4::x4_kernel::drivers::input::Event;

#[test]
fn r10_host_known_packets_match_stock_protocol() {
    assert_eq!(
        R10_REMOTE_START.bytes(),
        [0x02, 0x04, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x06]
    );

    assert_eq!(
        R10_REMOTE_POLL.bytes(),
        [0x02, 0x05, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x07]
    );

    assert_eq!(
        R10_REMOTE_STOP.bytes(),
        [0x02, 0x06, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x08]
    );
}

#[test]
fn r10_host_motion_packet_becomes_next_page() {
    let motion = [0x02, 0x02, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x04];

    assert_eq!(
        R10RemotePacket::classify(&motion),
        R10RemotePacketKind::RemoteMotion
    );

    let mut policy = R10RemotePolicy::enabled_for_reader(3500);
    assert_eq!(
        policy.handle_packet(10_000, &motion),
        R10RemoteAction::Reader(RustmixReaderAction::NextPage)
    );
}

#[test]
fn r10_host_motion_is_reader_only_and_debounced() {
    let motion = [0x02, 0x02, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x04];

    let mut policy = R10RemotePolicy::enabled_for_reader(3500);
    assert_eq!(
        policy.handle_packet(10_000, &motion),
        R10RemoteAction::Reader(RustmixReaderAction::NextPage)
    );
    assert_eq!(policy.handle_packet(11_000, &motion), R10RemoteAction::None);
    assert_eq!(
        policy.handle_packet(14_000, &motion),
        R10RemoteAction::Reader(RustmixReaderAction::NextPage)
    );

    let mut settings_policy = R10RemotePolicy {
        enabled: true,
        screen: R10RemoteScreen::Settings,
        debounce: policy.debounce,
    };
    assert_eq!(
        settings_policy.handle_packet(20_000, &motion),
        R10RemoteAction::None
    );
}

#[test]
fn r10_host_next_page_maps_to_existing_x4_next_button() {
    assert_eq!(
        R10InputInjection::from_remote_action(R10RemoteAction::Reader(
            RustmixReaderAction::NextPage
        )),
        R10InputInjection::Press(Event::Press(Button::VolDown))
    );
}

#[test]
fn r10_host_previous_page_maps_to_existing_x4_previous_button() {
    assert_eq!(
        R10InputInjection::from_remote_action(R10RemoteAction::Reader(
            RustmixReaderAction::PreviousPage
        )),
        R10InputInjection::Press(Event::Press(Button::VolUp))
    );
}
