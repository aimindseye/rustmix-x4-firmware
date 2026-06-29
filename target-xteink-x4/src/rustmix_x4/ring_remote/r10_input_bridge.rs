// Synthetic input bridge for COLMI R10 remote events.
//
// This module maps transport-independent R10 remote actions onto the existing
// X4 input path. It does not control ReaderApp directly. Instead it injects
// the same hardware input event used by the physical page buttons.

use crate::rustmix_x4::contracts::input_semantics::RustmixReaderAction;
use crate::rustmix_x4::ring_remote::r10_remote_policy::R10RemoteAction;
use crate::rustmix_x4::x4_kernel::board::button::Button;
use crate::rustmix_x4::x4_kernel::drivers::input::Event;
use crate::rustmix_x4::x4_kernel::kernel::tasks;

pub const R10_NEXT_PAGE_BUTTON: Button = Button::VolDown;
pub const R10_PREVIOUS_PAGE_BUTTON: Button = Button::VolUp;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R10InputInjection {
    None,
    Press(Event),
}

impl R10InputInjection {
    pub const fn from_remote_action(action: R10RemoteAction) -> Self {
        match action {
            R10RemoteAction::Reader(RustmixReaderAction::NextPage) => {
                Self::Press(Event::Press(R10_NEXT_PAGE_BUTTON))
            }
            R10RemoteAction::Reader(RustmixReaderAction::PreviousPage) => {
                Self::Press(Event::Press(R10_PREVIOUS_PAGE_BUTTON))
            }
            _ => Self::None,
        }
    }

    pub const fn event(self) -> Option<Event> {
        match self {
            Self::None => None,
            Self::Press(event) => Some(event),
        }
    }
}

pub fn try_enqueue_remote_action(action: R10RemoteAction) -> bool {
    match R10InputInjection::from_remote_action(action).event() {
        Some(event) => tasks::INPUT_EVENTS.try_send(event).is_ok(),
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn r10_next_page_maps_to_existing_next_button() {
        assert_eq!(
            R10InputInjection::from_remote_action(R10RemoteAction::Reader(
                RustmixReaderAction::NextPage
            )),
            R10InputInjection::Press(Event::Press(Button::VolDown))
        );
    }

    #[test]
    fn r10_previous_page_maps_to_existing_previous_button() {
        assert_eq!(
            R10InputInjection::from_remote_action(R10RemoteAction::Reader(
                RustmixReaderAction::PreviousPage
            )),
            R10InputInjection::Press(Event::Press(Button::VolUp))
        );
    }

    #[test]
    fn r10_none_action_does_not_enqueue_input() {
        assert_eq!(
            R10InputInjection::from_remote_action(R10RemoteAction::None),
            R10InputInjection::None
        );
    }
}
