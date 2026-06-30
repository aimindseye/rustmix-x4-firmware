#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R10BleLiveCentralStatus {
    Disabled,
    ProbeOnlyLogOnly,
    ReaderRemoteNeedsLiveStack,
}

impl R10BleLiveCentralStatus {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Disabled => "disabled",
            Self::ProbeOnlyLogOnly => "probe_only_log_only",
            Self::ReaderRemoteNeedsLiveStack => "reader_remote_needs_live_stack",
        }
    }

    pub const fn mode_label(self) -> &'static str {
        match self {
            Self::Disabled => "disabled",
            Self::ProbeOnlyLogOnly => "probe_only",
            Self::ReaderRemoteNeedsLiveStack => "reader_remote",
        }
    }

    pub const fn reader_label(self) -> &'static str {
        match self {
            Self::ReaderRemoteNeedsLiveStack => "reader_on",
            _ => "reader_off",
        }
    }
}

pub fn r10_ble_live_central_compiled_status() -> R10BleLiveCentralStatus {
    match super::r10_ble_x4_build_mode::r10_ble_x4_compiled_mode_opt().unwrap_or("probe_only") {
        "reader_remote" | "reader-remote" | "reader"
            if super::r10_ble_x4_build_mode::r10_ble_x4_compiled_allow_reader_remote_opt()
                == Some("1") =>
        {
            R10BleLiveCentralStatus::ReaderRemoteNeedsLiveStack
        }
        "off" | "disabled" => R10BleLiveCentralStatus::Disabled,
        _ => R10BleLiveCentralStatus::ProbeOnlyLogOnly,
    }
}

pub fn r10_ble_x4_emit_live_central_status_log() {
    let status = r10_ble_live_central_compiled_status();

    #[cfg(target_arch = "riscv32")]
    esp_println::println!(
        "rustmix event=ble_live_central kind=status mode={} reader={} status={}",
        status.mode_label(),
        status.reader_label(),
        status.label()
    );

    #[cfg(not(target_arch = "riscv32"))]
    std::println!(
        "rustmix event=ble_live_central kind=status mode={} reader={} status={}",
        status.mode_label(),
        status.reader_label(),
        status.label()
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn r10_ble_live_central_status_labels_are_safe() {
        assert_eq!(R10BleLiveCentralStatus::Disabled.label(), "disabled");
        assert_eq!(
            R10BleLiveCentralStatus::ProbeOnlyLogOnly.label(),
            "probe_only_log_only"
        );
        assert_eq!(
            R10BleLiveCentralStatus::ReaderRemoteNeedsLiveStack.label(),
            "reader_remote_needs_live_stack"
        );
        assert_eq!(
            R10BleLiveCentralStatus::ReaderRemoteNeedsLiveStack.reader_label(),
            "reader_on"
        );
    }
}
