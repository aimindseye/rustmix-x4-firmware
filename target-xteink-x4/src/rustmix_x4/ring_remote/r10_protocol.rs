// COLMI R10 stock BLE remote-photo/motion protocol.
//
// The R10 uses 16-byte packets on the normal UART-like service:
//   service: 6e40fff0-b5a3-f393-e0a9-e50e24dcca9e
//   write:   6e400002-b5a3-f393-e0a9-e50e24dcca9e
//   notify:  6e400003-b5a3-f393-e0a9-e50e24dcca9e
//
// Packet checksum:
//   byte 15 = sum(bytes 0..14) & 0xff
//
// Remote-photo/motion mode:
//   start: 02 04 ... 06
//   poll:  02 05 ... 07
//   stop:  02 06 ... 08
//
// Notify responses:
//   02 00 ... = no motion event
//   02 02 ... = motion event

pub const R10_PACKET_LEN: usize = 16;

pub const R10_CMD_REMOTE: u8 = 0x02;
pub const R10_REMOTE_EVENT_NONE: u8 = 0x00;
pub const R10_REMOTE_EVENT_MOTION: u8 = 0x02;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R10RemotePacketKind {
    RemoteStart,
    RemotePoll,
    RemoteStop,
    RemoteNoEvent,
    RemoteMotion,
    Unknown,
    BadLength,
    BadChecksum,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R10RemotePacket {
    bytes: [u8; R10_PACKET_LEN],
}

impl R10RemotePacket {
    pub const fn from_payload2(cmd: u8, arg: u8) -> Self {
        let mut bytes = [0u8; R10_PACKET_LEN];
        bytes[0] = cmd;
        bytes[1] = arg;
        bytes[15] = cmd.wrapping_add(arg);
        Self { bytes }
    }

    pub const fn bytes(&self) -> [u8; R10_PACKET_LEN] {
        self.bytes
    }

    pub const fn as_slice(&self) -> &[u8; R10_PACKET_LEN] {
        &self.bytes
    }

    pub fn checksum_for_prefix(prefix: &[u8; 15]) -> u8 {
        let mut sum = 0u8;
        let mut i = 0;
        while i < prefix.len() {
            sum = sum.wrapping_add(prefix[i]);
            i += 1;
        }
        sum
    }

    pub fn checksum_ok(bytes: &[u8]) -> bool {
        if bytes.len() != R10_PACKET_LEN {
            return false;
        }

        let mut sum = 0u8;
        for byte in &bytes[..15] {
            sum = sum.wrapping_add(*byte);
        }
        sum == bytes[15]
    }

    pub fn classify(bytes: &[u8]) -> R10RemotePacketKind {
        if bytes.len() != R10_PACKET_LEN {
            return R10RemotePacketKind::BadLength;
        }

        if !Self::checksum_ok(bytes) {
            return R10RemotePacketKind::BadChecksum;
        }

        match (bytes[0], bytes[1]) {
            (R10_CMD_REMOTE, 0x04) => R10RemotePacketKind::RemoteStart,
            (R10_CMD_REMOTE, 0x05) => R10RemotePacketKind::RemotePoll,
            (R10_CMD_REMOTE, 0x06) => R10RemotePacketKind::RemoteStop,
            (R10_CMD_REMOTE, R10_REMOTE_EVENT_NONE) => R10RemotePacketKind::RemoteNoEvent,
            (R10_CMD_REMOTE, R10_REMOTE_EVENT_MOTION) => R10RemotePacketKind::RemoteMotion,
            _ => R10RemotePacketKind::Unknown,
        }
    }
}

pub const R10_REMOTE_START: R10RemotePacket = R10RemotePacket::from_payload2(R10_CMD_REMOTE, 0x04);
pub const R10_REMOTE_POLL: R10RemotePacket = R10RemotePacket::from_payload2(R10_CMD_REMOTE, 0x05);
pub const R10_REMOTE_STOP: R10RemotePacket = R10RemotePacket::from_payload2(R10_CMD_REMOTE, 0x06);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn r10_known_packets_match_stock_protocol() {
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
    fn r10_classifies_motion_and_no_event() {
        let no_event = [0x02, 0x00, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x02];
        let motion = [0x02, 0x02, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x04];

        assert_eq!(
            R10RemotePacket::classify(&no_event),
            R10RemotePacketKind::RemoteNoEvent
        );
        assert_eq!(
            R10RemotePacket::classify(&motion),
            R10RemotePacketKind::RemoteMotion
        );
    }

    #[test]
    fn r10_rejects_bad_checksum() {
        let bad = [0x02, 0x02, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x05];
        assert_eq!(
            R10RemotePacket::classify(&bad),
            R10RemotePacketKind::BadChecksum
        );
    }
}
