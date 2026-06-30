// Settings app implementation.
//
// The active runtime already owns AppId::Settings.  This screen keeps the
// existing loaded SystemSettings available to the app manager, while the reader-preference work
// persists Settings rows through /RUSTMIX/SETTINGS.TXT.

use core::fmt::Write as _;

use embedded_graphics::{pixelcolor::BinaryColor, prelude::*, primitives::PrimitiveStyle};

use crate::rustmix_x4::ring_remote::{
    R10BleSettingsAction, R10BleSettingsController, R10BleSettingsMode,
};
use crate::rustmix_x4::ui::page_shell::DEFAULT_SETTINGS_TABS;
use crate::rustmix_x4::x4_apps::apps::{
    App, AppContext, AppId, PendingSetting, Transition, reader_state,
};
use crate::rustmix_x4::x4_apps::fonts;
use crate::rustmix_x4::x4_apps::ui::{
    Alignment, BUTTON_BAR_H, BitmapLabel, CONTENT_TOP, Region, StackFmt, wrap_next, wrap_prev,
};
use crate::rustmix_x4::x4_kernel::board::action::{Action, ActionEvent};
use crate::rustmix_x4::x4_kernel::board::{SCREEN_H, SCREEN_W};
use crate::rustmix_x4::x4_kernel::drivers::strip::StripBuffer;
use crate::rustmix_x4::x4_kernel::kernel::KernelHandle;
use crate::rustmix_x4::x4_kernel::kernel::config::{
    self, ReaderPreferences, SystemSettings, WifiConfig, parse_settings_txt, write_settings_txt,
};

const ROW_H: u16 = 40;
const ROW_GAP: u16 = 0;
const ROW_STRIDE: u16 = ROW_H + ROW_GAP;
const FIXED_INTERFACE_FONT_LABEL: &str = "Inter";
pub const SETTINGS_CROSSINK_VISUAL_MARKER: &str = "crossink-visual-parity-rustmix-ok";
pub const SETTINGS_CROSSINK_RENDERER_MARKER: &str = "crossink-settings-renderer-rustmix-ok";
pub const SETTINGS_R10_BLE_CONTROLS_ROW_MARKER: &str = "settings-controls-r10-ble-remote-row-ok";
pub const SETTINGS_R10_BLE_SECTION_LABEL: &str = "R10 BLE REMOTE";
pub const SETTINGS_R10_BLE_ROW_LABEL: &str = "R10 BLE Remote";
pub const SETTINGS_CROSSINK_VISUAL_PARITY_MARKER: &str =
    "crossink-settings-visual-parity-rustmix-ok";

// CrossInk Settings visual parity metrics. These mirror the CrossInk theme
// renderer model instead of Biscuit cards: compact top padding, fixed header,
// flowing tab strip, dense list rows, right-aligned values, and boxed hints.
const PAGE_X: u16 = 0;
const PAGE_W: u16 = SCREEN_W;
const CROSSINK_TOP_PADDING: u16 = 5;
const HEADER_Y: u16 = CROSSINK_TOP_PADDING;
const HEADER_H: u16 = 45;
const HEADER_TEXT_X: u16 = 20;
const STATUS_W: u16 = 96;
const TAB_Y: u16 = HEADER_Y + HEADER_H;
const TAB_H: u16 = 50;
const TAB_TEXT_X: u16 = 20;
const TAB_SPACING: u16 = 18;
const LIST_X: u16 = 0;
const LIST_TOP: u16 = TAB_Y + TAB_H + 10;
const LIST_W: u16 = SCREEN_W;
const CONTENT_PAD_X: u16 = 20;
const VALUE_W: u16 = 170;
const VALUE_PAD: u16 = 8;
const TAB_COUNT: u8 = DEFAULT_SETTINGS_TABS.len() as u8;
const TITLE_CACHE_ACTION_NONE: u8 = 0;
const TITLE_CACHE_ACTION_RELOAD: u8 = 1;
const TITLE_CACHE_ACTION_REBUILD: u8 = 2;

const TITLE_CACHE_MODE_COUNT: u8 = 2;
const TITLE_CACHE_MODE_LABELS: [&str; TITLE_CACHE_MODE_COUNT as usize] = ["Runtime", "Reset stale"];

const TITLE_CACHE_STATUS_READY: u8 = 0;
const TITLE_CACHE_STATUS_QUEUED: u8 = 1;
const TITLE_CACHE_STATUS_RUNNING: u8 = 2;
const TITLE_CACHE_STATUS_DONE: u8 = 3;
const TITLE_CACHE_STATUS_FAILED: u8 = 4;
const TITLE_CACHE_STATUS_LABELS: [&str; 5] = ["Run now", "Queued", "Running", "Run again", "Retry"];

#[derive(Clone, Copy, PartialEq, Eq)]
enum SettingsRowKind {
    Section(&'static str),
    StaticValue(&'static str),
    ReaderFont,
    ReaderTheme,
    ReaderOrientation,
    ReaderPreparedProfile,
    ReaderFallbackPolicy,
    ReaderProgress,
    ReaderSunlightFadingFix,
    DisplayRefresh,
    DisplayInvert,
    DisplayContrast,
    UiFontSource,
    StorageSdStatus,
    StorageBooksCount,
    StorageTitleCache,
    StorageRebuildCache,
    DeviceBattery,
    DeviceSleepTimeout,
    DeviceButtonTest,
    R10BleRemoteMode,
    AboutOs,
    AboutDevice,
    AboutBuild,
    AboutStorage,
}

#[derive(Clone, Copy)]
struct SettingsRow {
    label: &'static str,
    kind: SettingsRowKind,
}

const DISPLAY_ROWS: [SettingsRow; 6] = [
    SettingsRow {
        label: "Sleep Images",
        kind: SettingsRowKind::StaticValue("/RUSTMIX/SLEEP"),
    },
    SettingsRow {
        label: "Sleep Format",
        kind: SettingsRowKind::StaticValue("800x480 1-bit BMP"),
    },
    SettingsRow {
        label: "Refresh Frequency",
        kind: SettingsRowKind::DisplayRefresh,
    },
    SettingsRow {
        label: "UI Theme",
        kind: SettingsRowKind::StaticValue("Lyra"),
    },
    SettingsRow {
        label: "Recent Books View",
        kind: SettingsRowKind::StaticValue("List"),
    },
    SettingsRow {
        label: "Sunlight Fading Fix",
        kind: SettingsRowKind::ReaderSunlightFadingFix,
    },
];

const READER_ROWS: [SettingsRow; 12] = [
    SettingsRow {
        label: "Font Family",
        kind: SettingsRowKind::StaticValue("Bitter"),
    },
    SettingsRow {
        label: "Manage Fonts",
        kind: SettingsRowKind::StaticValue(""),
    },
    SettingsRow {
        label: "Font Size",
        kind: SettingsRowKind::ReaderFont,
    },
    SettingsRow {
        label: "Line Spacing",
        kind: SettingsRowKind::StaticValue("Wide"),
    },
    SettingsRow {
        label: "Orientation",
        kind: SettingsRowKind::ReaderOrientation,
    },
    SettingsRow {
        label: "Screen Margin",
        kind: SettingsRowKind::StaticValue("5"),
    },
    SettingsRow {
        label: "Paragraph Alignment",
        kind: SettingsRowKind::StaticValue("Justify"),
    },
    SettingsRow {
        label: "Embedded Style",
        kind: SettingsRowKind::StaticValue("ON"),
    },
    SettingsRow {
        label: "Hyphenation",
        kind: SettingsRowKind::StaticValue("ON"),
    },
    SettingsRow {
        label: "Images",
        kind: SettingsRowKind::StaticValue("Display"),
    },
    SettingsRow {
        label: "Bionic Reading",
        kind: SettingsRowKind::StaticValue("OFF"),
    },
    SettingsRow {
        label: "Guide Dots",
        kind: SettingsRowKind::StaticValue("OFF"),
    },
];

const CONTROLS_ROWS: [SettingsRow; 12] = [
    SettingsRow {
        label: SETTINGS_R10_BLE_SECTION_LABEL,
        kind: SettingsRowKind::Section(SETTINGS_R10_BLE_SECTION_LABEL),
    },
    SettingsRow {
        label: SETTINGS_R10_BLE_ROW_LABEL,
        kind: SettingsRowKind::R10BleRemoteMode,
    },
    SettingsRow {
        label: "POWER BUTTON",
        kind: SettingsRowKind::Section("POWER BUTTON"),
    },
    SettingsRow {
        label: "Short-press Action",
        kind: SettingsRowKind::StaticValue("Page Turn"),
    },
    SettingsRow {
        label: "Long-press Action",
        kind: SettingsRowKind::StaticValue("Sleep"),
    },
    SettingsRow {
        label: "FRONT BUTTONS",
        kind: SettingsRowKind::Section("FRONT BUTTONS"),
    },
    SettingsRow {
        label: "Remap Front Buttons",
        kind: SettingsRowKind::DeviceButtonTest,
    },
    SettingsRow {
        label: "Orientation Aware",
        kind: SettingsRowKind::StaticValue("No"),
    },
    SettingsRow {
        label: "Long-press Button Behavior",
        kind: SettingsRowKind::StaticValue("OFF"),
    },
    SettingsRow {
        label: "SIDE BUTTONS",
        kind: SettingsRowKind::Section("SIDE BUTTONS"),
    },
    SettingsRow {
        label: "Layout",
        kind: SettingsRowKind::StaticValue("Prev/Next"),
    },
    SettingsRow {
        label: "Long-press Action",
        kind: SettingsRowKind::StaticValue("Chapter Skip"),
    },
];

const SYSTEM_ROWS: [SettingsRow; 12] = [
    SettingsRow {
        label: "Storage",
        kind: SettingsRowKind::Section("Storage"),
    },
    SettingsRow {
        label: "SD Status",
        kind: SettingsRowKind::StorageSdStatus,
    },
    SettingsRow {
        label: "Books Count",
        kind: SettingsRowKind::StorageBooksCount,
    },
    SettingsRow {
        label: "Title Cache Mode",
        kind: SettingsRowKind::StorageTitleCache,
    },
    SettingsRow {
        label: "Title Cache Action",
        kind: SettingsRowKind::StorageRebuildCache,
    },
    SettingsRow {
        label: "Device",
        kind: SettingsRowKind::Section("Device"),
    },
    SettingsRow {
        label: "Battery",
        kind: SettingsRowKind::DeviceBattery,
    },
    SettingsRow {
        label: "About",
        kind: SettingsRowKind::Section("About"),
    },
    SettingsRow {
        label: "Rustmix",
        kind: SettingsRowKind::AboutOs,
    },
    SettingsRow {
        label: "Xteink X4",
        kind: SettingsRowKind::AboutDevice,
    },
    SettingsRow {
        label: "Build",
        kind: SettingsRowKind::AboutBuild,
    },
    SettingsRow {
        label: "Storage Layout",
        kind: SettingsRowKind::AboutStorage,
    },
];

impl Default for SettingsApp {
    fn default() -> Self {
        Self::new()
    }
}

pub struct SettingsApp {
    settings: SystemSettings,
    r10_ble_settings: R10BleSettingsController,
    wifi: WifiConfig,
    selected_tab: u8,
    selected: usize,
    scroll: usize,
    focus_tabs: bool,
    loaded: bool,
    save_needed: bool,
    ui_fonts: fonts::UiFonts,
    rows_top: u16,

    reader_font: u8,
    reader_theme: u8,
    reader_orientation: u8,
    reader_prepared_profile: u8,
    reader_fallback_policy: u8,
    reader_show_progress: bool,
    display_refresh: u8,
    display_invert: bool,
    display_contrast: bool,
    ui_font_source: u8,
    device_sleep_timeout: u8,
    device_battery_mv: u16,
    title_cache_mode: u8,
    title_cache_status: u8,
    title_cache_action: u8,
}

impl SettingsApp {
    pub fn new() -> Self {
        let uf = fonts::UiFonts::for_size(0);
        Self {
            settings: SystemSettings::defaults(),
            r10_ble_settings: R10BleSettingsController::new(),
            wifi: WifiConfig::empty(),
            selected_tab: 0,
            selected: 0,
            scroll: 0,
            focus_tabs: true,
            loaded: false,
            save_needed: false,
            ui_fonts: uf,
            rows_top: LIST_TOP,
            reader_font: config::DEFAULT_FONT_SIZE_IDX,
            reader_theme: config::DEFAULT_READING_THEME,
            reader_orientation: config::DEFAULT_READER_ORIENTATION,
            reader_prepared_profile: config::DEFAULT_PREPARED_FONT_PROFILE,
            reader_fallback_policy: config::DEFAULT_PREPARED_FALLBACK_POLICY,
            reader_show_progress: true,
            display_refresh: 1,
            display_invert: false,
            display_contrast: false,
            ui_font_source: 1,
            device_sleep_timeout: 1,
            device_battery_mv: 0,
            title_cache_mode: 0,
            title_cache_status: TITLE_CACHE_STATUS_READY,
            title_cache_action: TITLE_CACHE_ACTION_NONE,
        }
    }

    pub fn set_ui_font_style(&mut self, _source: u8, _idx: u8) {
        self.ui_fonts = fonts::UiFonts::for_size(0);
    }

    pub fn set_ui_font_size(&mut self, _idx: u8) {
        self.ui_fonts = fonts::UiFonts::for_size(0);
    }

    pub fn system_settings(&self) -> &SystemSettings {
        &self.settings
    }

    pub fn system_settings_mut(&mut self) -> &mut SystemSettings {
        &mut self.settings
    }

    pub fn wifi_config(&self) -> &WifiConfig {
        &self.wifi
    }

    pub fn selected_tab(&self) -> u8 {
        self.selected_tab.min(TAB_COUNT.saturating_sub(1))
    }

    pub fn focus_tabs(&self) -> bool {
        self.focus_tabs
    }

    pub fn mark_save_needed(&mut self) {
        self.save_needed = true;
    }

    pub fn is_loaded(&self) -> bool {
        self.loaded
    }

    pub fn load_eager(&mut self, k: &mut KernelHandle<'_>) {
        self.load(k);
        self.set_ui_font_size(self.settings.ui_font_size_idx);
    }

    fn load(&mut self, k: &mut KernelHandle<'_>) {
        let mut buf = [0u8; 1024];
        self.device_battery_mv = k.battery_mv();

        self.settings = SystemSettings::defaults();
        self.wifi = WifiConfig::empty();

        match k.read_app_data_start(config::SETTINGS_FILE, &mut buf) {
            Ok((_size, n)) if n > 0 => {
                parse_settings_txt(&buf[..n], &mut self.settings, &mut self.wifi);
                self.settings.sanitize();
                log::info!("settings: loaded from {}", config::SETTINGS_FILE);
            }
            _ => {
                log::info!("settings: no file found, using defaults");
            }
        }

        self.sync_local_from_system_settings();
        self.loaded = true;
    }

    fn read_wifi_credentials_from_settings_file(k: &mut KernelHandle<'_>) -> WifiConfig {
        let mut buf = [0u8; 1024];
        let mut settings = SystemSettings::defaults();
        let mut wifi = WifiConfig::empty();

        if let Ok((_size, n)) = k.read_app_data_start(config::SETTINGS_FILE, &mut buf) {
            if n > 0 {
                parse_settings_txt(&buf[..n], &mut settings, &mut wifi);
            }
        }

        wifi
    }

    fn preserve_wifi_credentials_before_save(&mut self, k: &mut KernelHandle<'_>) {
        if self.wifi.has_credentials() && !self.wifi.password().is_empty() {
            return;
        }

        let existing = Self::read_wifi_credentials_from_settings_file(k);
        if existing.has_credentials() && !existing.password().is_empty() {
            self.wifi = existing;
        }
    }

    fn queue_title_cache_action(&mut self, action: u8) {
        self.title_cache_action = action;
        self.title_cache_status = TITLE_CACHE_STATUS_QUEUED;
    }

    fn run_title_cache_action(&mut self, k: &mut KernelHandle<'_>) {
        let action = self.title_cache_action;
        if action == TITLE_CACHE_ACTION_NONE {
            return;
        }

        self.title_cache_action = TITLE_CACHE_ACTION_NONE;
        self.title_cache_status = TITLE_CACHE_STATUS_RUNNING;

        let result = match action {
            TITLE_CACHE_ACTION_RELOAD => {
                k.invalidate_dir_cache();
                k.ensure_dir_cache_loaded().map(|_| ())
            }
            TITLE_CACHE_ACTION_REBUILD => {
                let _ = k.delete_cache(crate::rustmix_x4::x4_kernel::drivers::storage::TITLES_FILE);
                k.invalidate_dir_cache();
                k.ensure_dir_cache_loaded().map(|_| ())
            }
            _ => Ok(()),
        };

        match result {
            Ok(()) => {
                self.title_cache_status = TITLE_CACHE_STATUS_DONE;
            }
            Err(e) => {
                log::warn!("settings: title cache action failed: {}", e);
                self.title_cache_status = TITLE_CACHE_STATUS_FAILED;
            }
        }
    }

    fn sync_local_from_system_settings(&mut self) {
        self.reader_font = self.settings.book_font_size_idx.min(fonts::max_size_idx());
        self.reader_theme = self
            .settings
            .reading_theme
            .min(reader_theme_count().saturating_sub(1));
        self.reader_prepared_profile = self
            .settings
            .prepared_font_profile
            .min(config::PREPARED_FONT_PROFILE_COUNT - 1);
        self.reader_fallback_policy = self
            .settings
            .prepared_fallback_policy
            .min(config::PREPARED_FALLBACK_POLICY_COUNT - 1);
        self.reader_show_progress = self.settings.reader_show_progress;
        self.reader_orientation = self
            .settings
            .reader_orientation
            .min(config::READER_ORIENTATION_COUNT - 1);
        self.display_refresh = self.settings.display_refresh_mode.min(2);
        self.display_invert = self.settings.display_invert_colors;
        self.display_contrast = self.settings.display_contrast_high;
        // Design option 1: OS chrome uses fixed Inter. The persisted legacy
        // UI font source is ignored so Settings/Home do not mix font metrics.
        self.ui_font_source = 1;
        self.device_sleep_timeout = match self.settings.sleep_timeout {
            0 => 3,
            1..=5 => 0,
            6..=10 => 1,
            _ => 2,
        };
    }

    fn sync_system_from_local(&mut self) {
        self.settings.set_reader_preferences(ReaderPreferences {
            book_font: self.reader_font.min(fonts::max_size_idx()),
            reading_theme: self
                .reader_theme
                .min(reader_theme_count().saturating_sub(1)),
            show_progress: self.reader_show_progress,
            sunlight_fading_fix: self.settings.reader_sunlight_fading_fix,
            reader_orientation: self
                .reader_orientation
                .min(config::READER_ORIENTATION_COUNT - 1),
            prepared_font_profile: self
                .reader_prepared_profile
                .min(config::PREPARED_FONT_PROFILE_COUNT - 1),
            prepared_fallback_policy: self
                .reader_fallback_policy
                .min(config::PREPARED_FALLBACK_POLICY_COUNT - 1),
            bionic_mode: self
                .settings
                .reader_bionic_mode
                .min(config::READER_BIONIC_MODE_COUNT - 1),
            guide_dots_mode: self
                .settings
                .reader_guide_dots_mode
                .min(config::READER_GUIDE_DOTS_MODE_COUNT - 1),
            reader_font_source: self.settings.reader_font_source,
            reader_sd_font_slot: self.settings.reader_sd_font_slot,
            reader_sd_font_id: self.settings.reader_sd_font_id,
            reader_sd_font_id_len: self.settings.reader_sd_font_id_len,
        });
        self.settings.display_refresh_mode = self.display_refresh.min(2);
        self.settings.display_invert_colors = self.display_invert;
        self.settings.display_contrast_high = self.display_contrast;
        self.settings.ui_font_source = 1;
        self.settings.sleep_timeout = match self.device_sleep_timeout {
            0 => 5,
            1 => 10,
            2 => 30,
            _ => 0,
        };
        self.settings.sanitize();
    }

    fn save(&mut self, k: &mut KernelHandle<'_>) -> bool {
        let mut buf = [0u8; 1024];
        self.preserve_wifi_credentials_before_save(k);
        let len = write_settings_txt(&self.settings, &self.wifi, &mut buf);
        let settings_saved = match k.write_app_data(config::SETTINGS_FILE, &buf[..len]) {
            Ok(_) => {
                log::info!("settings: saved to {}", config::SETTINGS_FILE);
                true
            }
            Err(e) => {
                log::error!("settings: save failed: {}", e);
                false
            }
        };

        settings_saved
    }

    fn active_rows(&self) -> &'static [SettingsRow] {
        match self.selected_tab.min(TAB_COUNT.saturating_sub(1)) {
            0 => &DISPLAY_ROWS,
            1 => &READER_ROWS,
            2 => &CONTROLS_ROWS,
            _ => &SYSTEM_ROWS,
        }
    }

    fn active_row_count(&self) -> usize {
        self.active_rows().len()
    }

    fn visible_rows(&self) -> usize {
        let footer_top = SCREEN_H.saturating_sub(BUTTON_BAR_H + 10);
        let avail = footer_top.saturating_sub(self.rows_top);
        let count = (avail / ROW_STRIDE) as usize;
        count.clamp(1, self.active_row_count().max(1))
    }

    fn shell_list_region(&self) -> Region {
        let footer_top = SCREEN_H.saturating_sub(BUTTON_BAR_H + 10);
        Region::new(
            LIST_X,
            self.rows_top,
            LIST_W,
            footer_top.saturating_sub(self.rows_top),
        )
    }

    fn row_region(&self, visible_idx: usize) -> Region {
        let list = self.shell_list_region();
        Region::new(
            list.x,
            self.rows_top + visible_idx as u16 * ROW_STRIDE,
            list.w,
            ROW_H,
        )
    }

    fn label_region(&self, visible_idx: usize) -> Region {
        let row = self.row_region(visible_idx);
        Region::new(
            row.x + CONTENT_PAD_X,
            row.y,
            row.w.saturating_sub(VALUE_W + CONTENT_PAD_X + VALUE_PAD),
            row.h,
        )
    }

    fn value_region(&self, visible_idx: usize) -> Region {
        let row = self.row_region(visible_idx);
        Region::new(
            row.x + row.w.saturating_sub(VALUE_W + CONTENT_PAD_X),
            row.y + 3,
            VALUE_W,
            row.h.saturating_sub(6),
        )
    }

    fn list_region(&self) -> Region {
        let list = self.shell_list_region();
        Region::new(
            list.x,
            self.rows_top,
            list.w,
            self.visible_rows() as u16 * ROW_STRIDE,
        )
    }

    fn active_selected_row_region(&self) -> Region {
        self.row_region(self.selected.saturating_sub(self.scroll))
    }

    fn scroll_into_view(&mut self) {
        let visible = self.visible_rows();
        if self.selected < self.scroll {
            self.scroll = self.selected;
        } else if self.selected >= self.scroll.saturating_add(visible) {
            self.scroll = self.selected.saturating_sub(visible.saturating_sub(1));
        }

        let max_scroll = self.active_row_count().saturating_sub(visible);
        self.scroll = self.scroll.min(max_scroll);
    }

    fn move_next(&mut self, ctx: &mut AppContext) {
        if self.focus_tabs {
            self.focus_tabs = false;
            self.selected = 0;
            self.scroll = 0;
            ctx.request_full_redraw();
            return;
        }
        self.move_to(wrap_next(self.selected, self.active_row_count()), ctx);
    }

    fn move_prev(&mut self, ctx: &mut AppContext) {
        if self.focus_tabs {
            self.focus_tabs = false;
            self.selected = self.active_row_count().saturating_sub(1);
            self.scroll_into_view();
            ctx.request_full_redraw();
            return;
        }
        self.move_to(wrap_prev(self.selected, self.active_row_count()), ctx);
    }

    fn move_to(&mut self, selected: usize, ctx: &mut AppContext) {
        let old_selected = self.selected;
        let old_scroll = self.scroll;
        let max_index = self.active_row_count().saturating_sub(1);
        self.focus_tabs = false;
        self.selected = selected.min(max_index);
        self.scroll_into_view();

        if self.scroll != old_scroll {
            ctx.mark_dirty(self.list_region());
        } else if self.selected != old_selected {
            ctx.mark_dirty(self.row_region(old_selected.saturating_sub(old_scroll)));
            ctx.mark_dirty(self.row_region(self.selected.saturating_sub(self.scroll)));
        }
    }

    fn switch_tab(&mut self, delta: isize, ctx: &mut AppContext) {
        let old_tab = self.selected_tab;
        self.selected_tab = cycle_index(self.selected_tab, TAB_COUNT, delta);
        if self.selected_tab != old_tab {
            self.selected = 0;
            self.scroll = 0;
            self.focus_tabs = true;
            ctx.request_full_redraw();
        }
    }

    fn cycle_selected(&mut self, delta: isize, ctx: &mut AppContext) {
        if self.focus_tabs {
            self.switch_tab(delta, ctx);
            return;
        }
        let row = self.active_rows()[self.selected];
        let changed = match row.kind {
            SettingsRowKind::ReaderFont => {
                self.reader_font =
                    cycle_index(self.reader_font, fonts::FONT_SIZE_COUNT as u8, delta);
                true
            }
            SettingsRowKind::ReaderTheme => {
                self.reader_theme = cycle_index(self.reader_theme, reader_theme_count(), delta);
                true
            }
            SettingsRowKind::ReaderOrientation => {
                self.reader_orientation = cycle_index(
                    self.reader_orientation,
                    config::READER_ORIENTATION_COUNT,
                    delta,
                );
                true
            }
            SettingsRowKind::ReaderPreparedProfile => {
                self.reader_prepared_profile = cycle_index(
                    self.reader_prepared_profile,
                    config::PREPARED_FONT_PROFILE_COUNT,
                    delta,
                );
                true
            }
            SettingsRowKind::ReaderFallbackPolicy => {
                self.reader_fallback_policy = cycle_index(
                    self.reader_fallback_policy,
                    config::PREPARED_FALLBACK_POLICY_COUNT,
                    delta,
                );
                true
            }
            SettingsRowKind::ReaderProgress => {
                self.reader_show_progress = !self.reader_show_progress;
                true
            }
            SettingsRowKind::ReaderSunlightFadingFix => {
                self.settings.reader_sunlight_fading_fix =
                    !self.settings.reader_sunlight_fading_fix;
                true
            }
            SettingsRowKind::DisplayRefresh => {
                self.display_refresh = cycle_index(self.display_refresh, 3, delta);
                true
            }
            SettingsRowKind::DisplayInvert => {
                self.display_invert = !self.display_invert;
                true
            }
            SettingsRowKind::DisplayContrast => {
                self.display_contrast = !self.display_contrast;
                true
            }
            SettingsRowKind::R10BleRemoteMode => self.cycle_r10_ble_remote_mode(ctx),
            SettingsRowKind::UiFontSource | SettingsRowKind::StaticValue(_) => false,
            SettingsRowKind::DeviceSleepTimeout => {
                self.device_sleep_timeout = cycle_index(self.device_sleep_timeout, 4, delta);
                true
            }
            SettingsRowKind::StorageTitleCache => {
                self.title_cache_mode =
                    cycle_index(self.title_cache_mode, TITLE_CACHE_MODE_COUNT, delta);
                self.title_cache_status = TITLE_CACHE_STATUS_READY;
                true
            }
            SettingsRowKind::StorageRebuildCache => {
                let action = if self.title_cache_mode == 0 {
                    TITLE_CACHE_ACTION_RELOAD
                } else {
                    TITLE_CACHE_ACTION_REBUILD
                };
                self.queue_title_cache_action(action);
                true
            }
            _ => false,
        };

        if changed {
            self.sync_system_from_local();
            self.save_needed = true;
        }
        if changed && matches!(row.kind, SettingsRowKind::UiFontSource) {
            ctx.request_full_redraw();
        } else {
            ctx.mark_dirty(self.active_selected_row_region());
        }
    }

    fn cycle_r10_ble_remote_mode(&mut self, ctx: &mut AppContext) -> bool {
        let action = match (
            self.r10_ble_settings.mode(),
            self.r10_ble_settings.pending_confirmation(),
        ) {
            (_, Some(R10BleSettingsMode::ReaderRemote)) => {
                R10BleSettingsAction::ConfirmReaderRemote
            }
            (R10BleSettingsMode::Off, _) => R10BleSettingsAction::SelectProbeOnly,
            (R10BleSettingsMode::ProbeOnly, _) => R10BleSettingsAction::SelectReaderRemote,
            (R10BleSettingsMode::ReaderRemote, _) => R10BleSettingsAction::QuickDisable,
        };

        let _transition = self.r10_ble_settings.apply(action);

        ctx.request_full_redraw();
        true
    }

    fn format_value(&self, kind: SettingsRowKind, buf: &mut StackFmt<40>) {
        buf.clear();
        match kind {
            SettingsRowKind::Section(name) => {
                let _ = write!(buf, "{}", name);
            }
            SettingsRowKind::StaticValue(value) => {
                let _ = write!(buf, "{}", value);
            }
            SettingsRowKind::ReaderFont => {
                let _ = write!(buf, "{}", fonts::font_size_name(self.reader_font));
            }
            SettingsRowKind::ReaderTheme => {
                let _ = write!(buf, "{}", reader_theme_name(self.reader_theme));
            }
            SettingsRowKind::ReaderOrientation => {
                let idx = self
                    .reader_orientation
                    .min(config::READER_ORIENTATION_COUNT - 1) as usize;
                let _ = write!(buf, "{}", config::READER_ORIENTATION_LABELS[idx]);
            }
            SettingsRowKind::ReaderPreparedProfile => {
                let idx =
                    self.reader_prepared_profile
                        .min(config::PREPARED_FONT_PROFILE_COUNT - 1) as usize;
                let _ = write!(buf, "{}", config::PREPARED_FONT_PROFILE_LABELS[idx]);
            }
            SettingsRowKind::ReaderFallbackPolicy => {
                let idx = self
                    .reader_fallback_policy
                    .min(config::PREPARED_FALLBACK_POLICY_COUNT - 1)
                    as usize;
                let _ = write!(buf, "{}", config::PREPARED_FALLBACK_POLICY_LABELS[idx]);
            }
            SettingsRowKind::ReaderProgress => {
                let _ = write!(buf, "{}", on_off(self.reader_show_progress));
            }
            SettingsRowKind::ReaderSunlightFadingFix => {
                let _ = write!(buf, "{}", on_off(self.settings.reader_sunlight_fading_fix));
            }
            SettingsRowKind::DisplayRefresh => {
                let _ = write!(
                    buf,
                    "{}",
                    ["Every page", "15 pages", "Fast"][self.display_refresh as usize]
                );
            }
            SettingsRowKind::DisplayInvert => {
                let _ = write!(buf, "{}", on_off(self.display_invert));
            }
            SettingsRowKind::UiFontSource => {
                let _ = write!(buf, "{}", FIXED_INTERFACE_FONT_LABEL);
            }
            SettingsRowKind::DisplayContrast => {
                let _ = write!(
                    buf,
                    "{}",
                    if self.display_contrast {
                        "High"
                    } else {
                        "Normal"
                    }
                );
            }
            SettingsRowKind::StorageSdStatus => {
                let _ = write!(buf, "Ready");
            }
            SettingsRowKind::StorageBooksCount => {
                let _ = write!(buf, "Library");
            }
            SettingsRowKind::StorageTitleCache => {
                let _ = write!(buf, "{}", title_cache_mode_name(self.title_cache_mode));
            }
            SettingsRowKind::StorageRebuildCache => {
                let _ = write!(buf, "{}", title_cache_status_name(self.title_cache_status));
            }
            SettingsRowKind::DeviceBattery => {
                if let Some(pct) = battery_percent_value(self.device_battery_mv) {
                    let _ = write!(buf, "{}% {}mV", pct, self.device_battery_mv);
                } else {
                    let _ = write!(buf, "--");
                }
            }
            SettingsRowKind::DeviceSleepTimeout => {
                let _ = write!(
                    buf,
                    "{}",
                    ["5 min", "10 min", "30 min", "Never"][self.device_sleep_timeout as usize]
                );
            }
            SettingsRowKind::DeviceButtonTest => {
                let _ = write!(buf, "Coming soon");
            }
            SettingsRowKind::R10BleRemoteMode => {
                if self.r10_ble_settings.pending_confirmation()
                    == Some(R10BleSettingsMode::ReaderRemote)
                {
                    let _ = buf.write_str("Confirm ReaderRemote");
                } else {
                    let _ = buf.write_str(self.r10_ble_settings.mode().status_label());
                }
            }
            SettingsRowKind::AboutOs => {
                let _ = write!(buf, "Rustmix");
            }
            SettingsRowKind::AboutDevice => {
                let _ = write!(buf, "Xteink X4");
            }
            SettingsRowKind::AboutBuild => {
                let _ = write!(buf, "riscv32 release");
            }
            SettingsRowKind::AboutStorage => {
                let _ = write!(buf, "RUSTMIX + TITLES.BIN");
            }
        }
    }
}

impl App<AppId> for SettingsApp {
    fn on_enter(&mut self, ctx: &mut AppContext, k: &mut KernelHandle<'_>) {
        self.device_battery_mv = k.battery_mv();
        if self.loaded {
            self.sync_local_from_system_settings();
        }
        self.selected = self.selected.min(self.active_row_count().saturating_sub(1));
        self.scroll = 0;
        self.focus_tabs = true;
        ctx.mark_dirty(Region::new(
            0,
            CONTENT_TOP,
            SCREEN_W,
            SCREEN_H - CONTENT_TOP,
        ));
    }

    fn on_event(&mut self, event: ActionEvent, ctx: &mut AppContext) -> Transition {
        match event {
            ActionEvent::Press(Action::Back) | ActionEvent::LongPress(Action::Back) => {
                if self.focus_tabs {
                    Transition::Pop
                } else {
                    self.focus_tabs = true;
                    ctx.request_full_redraw();
                    Transition::None
                }
            }
            ActionEvent::Press(Action::Next) | ActionEvent::Repeat(Action::Next) => {
                self.move_next(ctx);
                Transition::None
            }
            ActionEvent::Press(Action::Prev) | ActionEvent::Repeat(Action::Prev) => {
                self.move_prev(ctx);
                Transition::None
            }
            ActionEvent::Press(Action::Select) => {
                self.cycle_selected(1, ctx);
                Transition::None
            }
            ActionEvent::LongPress(Action::Select) => {
                self.cycle_selected(-1, ctx);
                Transition::None
            }
            ActionEvent::Press(Action::NextJump) | ActionEvent::Repeat(Action::NextJump) => {
                self.switch_tab(1, ctx);
                Transition::None
            }
            ActionEvent::Press(Action::PrevJump) | ActionEvent::Repeat(Action::PrevJump) => {
                self.switch_tab(-1, ctx);
                Transition::None
            }
            _ => Transition::None,
        }
    }

    async fn background(&mut self, ctx: &mut AppContext, k: &mut KernelHandle<'_>) {
        let battery_mv = k.battery_mv();
        if battery_mv != self.device_battery_mv {
            self.device_battery_mv = battery_mv;
            if self.loaded {
                ctx.request_full_redraw();
            }
        }

        if !self.loaded {
            self.load(k);
            ctx.request_full_redraw();
            return;
        }

        if self.title_cache_action != TITLE_CACHE_ACTION_NONE {
            self.run_title_cache_action(k);
            ctx.mark_dirty(self.active_selected_row_region());
        }

        if self.save_needed && self.save(k) {
            self.save_needed = false;
        }
    }

    fn pending_setting(&self) -> Option<PendingSetting> {
        Some(PendingSetting::ReaderPreferences(
            self.settings.reader_preferences(),
        ))
    }

    fn has_background_when_suspended(&self) -> bool {
        self.save_needed || self.title_cache_action != TITLE_CACHE_ACTION_NONE
    }

    fn background_suspended(&mut self, k: &mut KernelHandle<'_>) {
        if self.title_cache_action != TITLE_CACHE_ACTION_NONE {
            self.run_title_cache_action(k);
        }

        if self.save_needed && self.save(k) {
            self.save_needed = false;
        }
    }

    fn draw(&self, strip: &mut StripBuffer) {
        self.draw_page_chrome(strip);

        if !self.loaded {
            BitmapLabel::new(
                self.row_region(0),
                "Loading...",
                fonts::ui_list_font_fixed(),
            )
            .alignment(Alignment::CenterLeft)
            .draw(strip)
            .unwrap();
            return;
        }

        self.draw_rows(strip);
        self.draw_scrollbar(strip);
    }
}

impl SettingsApp {
    fn shell_section_font(&self) -> &'static fonts::bitmap::BitmapFont {
        fonts::ui_list_section_font_fixed()
    }

    fn draw_page_chrome(&self, strip: &mut StripBuffer) {
        draw_region_fill(
            strip,
            Region::new(PAGE_X, PAGE_X, PAGE_W, SCREEN_H),
            BinaryColor::Off,
        );

        let title_region = Region::new(
            HEADER_TEXT_X,
            HEADER_Y + 5,
            SCREEN_W.saturating_sub(HEADER_TEXT_X + STATUS_W + 16),
            HEADER_H.saturating_sub(10),
        );
        BitmapLabel::new(title_region, "Settings", self.ui_fonts.heading)
            .alignment(Alignment::CenterLeft)
            .semibold()
            .draw(strip)
            .unwrap();

        let status_region = Region::new(
            SCREEN_W.saturating_sub(STATUS_W + 16),
            HEADER_Y + 5,
            STATUS_W,
            self.ui_fonts.body.line_height,
        );
        let mut status = StackFmt::<24>::new();
        if let Some(pct) = battery_percent_value(self.device_battery_mv) {
            let _ = write!(status, "{}%", pct);
        } else {
            let _ = write!(status, "x4");
        }
        BitmapLabel::new(status_region, status.as_str(), fonts::chrome_font())
            .alignment(Alignment::CenterRight)
            .medium()
            .draw(strip)
            .unwrap();

        draw_region_fill(
            strip,
            Region::new(PAGE_X, TAB_Y.saturating_sub(1), PAGE_W, 1),
            BinaryColor::On,
        );
        self.draw_tab_bar(strip);
    }

    fn draw_tab_bar(&self, strip: &mut StripBuffer) {
        let tab_font = fonts::ui_list_font_fixed();
        let bar = Region::new(PAGE_X, TAB_Y, PAGE_W, TAB_H);

        draw_region_dither(strip, bar);
        draw_region_fill(
            strip,
            Region::new(PAGE_X, TAB_Y + TAB_H - 1, PAGE_W, 1),
            BinaryColor::On,
        );

        let mut x = TAB_TEXT_X;
        for (idx, label) in DEFAULT_SETTINGS_TABS.iter().enumerate() {
            let selected = idx as u8 == self.selected_tab;
            let text_w = tab_font.measure_str(label).saturating_add(8);
            let label_region = Region::new(x, TAB_Y + 6, text_w, TAB_H.saturating_sub(12));

            if selected {
                if self.focus_tabs {
                    draw_region_fill(strip, label_region, BinaryColor::On);
                } else {
                    let underline_y = label_region
                        .y
                        .saturating_add(label_region.h.saturating_sub(3));
                    draw_region_fill(
                        strip,
                        Region::new(x + 3, underline_y, text_w.saturating_sub(6), 2),
                        BinaryColor::On,
                    );
                }
            }

            BitmapLabel::new(label_region, label, tab_font)
                .alignment(Alignment::Center)
                .inverted(selected && self.focus_tabs)
                .weight(if selected {
                    crate::rustmix_x4::x4_apps::ui::BitmapTextWeight::SemiBold
                } else {
                    crate::rustmix_x4::x4_apps::ui::BitmapTextWeight::Medium
                })
                .draw(strip)
                .unwrap();

            x = x.saturating_add(text_w).saturating_add(TAB_SPACING);
        }
    }

    fn draw_rows(&self, strip: &mut StripBuffer) {
        let rows = self.active_rows();
        let visible = self
            .visible_rows()
            .min(rows.len().saturating_sub(self.scroll));
        let mut val_buf = StackFmt::<40>::new();

        for vi in 0..visible {
            let idx = self.scroll + vi;
            let row = rows[idx];
            let selected = !self.focus_tabs && idx == self.selected;
            let is_section = matches!(row.kind, SettingsRowKind::Section(_));
            let row_region = self.row_region(vi);

            draw_region_fill(strip, row_region, BinaryColor::Off);

            if selected && !is_section {
                // CrossInk list focus is row-based. Rustmix keeps the setting value
                // pill behavior for editable values, but only when the list has focus.
            }

            if is_section {
                BitmapLabel::new(self.label_region(vi), row.label, self.shell_section_font())
                    .alignment(Alignment::CenterLeft)
                    .semibold()
                    .draw(strip)
                    .unwrap();
                continue;
            }

            BitmapLabel::new(
                self.label_region(vi),
                row.label,
                fonts::ui_list_font_fixed(),
            )
            .alignment(Alignment::CenterLeft)
            .medium()
            .draw(strip)
            .unwrap();

            self.format_value(row.kind, &mut val_buf);
            let value_region = self.value_region(vi);
            if selected && !val_buf.as_str().is_empty() {
                draw_region_fill(strip, value_region, BinaryColor::On);
                BitmapLabel::new(value_region, val_buf.as_str(), fonts::ui_list_font_fixed())
                    .alignment(Alignment::CenterRight)
                    .inverted(true)
                    .semibold()
                    .draw(strip)
                    .unwrap();
            } else {
                BitmapLabel::new(value_region, val_buf.as_str(), fonts::ui_list_font_fixed())
                    .alignment(Alignment::CenterRight)
                    .medium()
                    .draw(strip)
                    .unwrap();
            }
        }
    }

    fn draw_scrollbar(&self, strip: &mut StripBuffer) {
        let rows = self.active_row_count();
        if rows <= self.visible_rows() {
            return;
        }
        let list = self.shell_list_region();
        let track = Region::new(SCREEN_W.saturating_sub(10), list.y, 3, list.h);
        draw_region_stroke(strip, track, BinaryColor::On);

        let thumb_h = ((track.h as usize * self.visible_rows()) / rows).max(12) as u16;
        let max_scroll = rows.saturating_sub(self.visible_rows()).max(1);
        let travel = track.h.saturating_sub(thumb_h);
        let thumb_y = track
            .y
            .saturating_add(((travel as usize * self.scroll) / max_scroll) as u16);
        let thumb = Region::new(track.x, thumb_y, track.w, thumb_h);
        draw_region_fill(strip, thumb, BinaryColor::On);
    }
}

fn draw_region_dither(strip: &mut StripBuffer, region: Region) {
    for y in region.y..region.y.saturating_add(region.h) {
        for x in region.x..region.x.saturating_add(region.w) {
            if ((x + y) & 1) == 0 {
                Pixel(Point::new(x as i32, y as i32), BinaryColor::On)
                    .draw(strip)
                    .unwrap();
            }
        }
    }
}

fn draw_region_fill(strip: &mut StripBuffer, region: Region, color: BinaryColor) {
    region
        .to_rect()
        .into_styled(PrimitiveStyle::with_fill(color))
        .draw(strip)
        .unwrap();
}

fn draw_region_stroke(strip: &mut StripBuffer, region: Region, color: BinaryColor) {
    region
        .to_rect()
        .into_styled(PrimitiveStyle::with_stroke(color, 1))
        .draw(strip)
        .unwrap();
}

fn title_cache_mode_name(idx: u8) -> &'static str {
    TITLE_CACHE_MODE_LABELS
        .get(idx as usize)
        .copied()
        .unwrap_or("Runtime")
}

fn title_cache_status_name(idx: u8) -> &'static str {
    TITLE_CACHE_STATUS_LABELS
        .get(idx as usize)
        .copied()
        .unwrap_or("Run now")
}

fn cycle_index(value: u8, count: u8, delta: isize) -> u8 {
    if count == 0 {
        return 0;
    }
    (value as isize + delta).rem_euclid(count as isize) as u8
}

fn reader_theme_count() -> u8 {
    reader_state::THEME_NAMES.len() as u8
}

fn reader_theme_name(idx: u8) -> &'static str {
    reader_state::THEME_NAMES
        .get(idx as usize)
        .copied()
        .unwrap_or("Default")
}

fn battery_percent_value(mv: u16) -> Option<u8> {
    if mv == 0 {
        None
    } else {
        Some(crate::rustmix_x4::x4_kernel::drivers::battery::battery_percentage(mv))
    }
}

const fn on_off(value: bool) -> &'static str {
    if value { "On" } else { "Off" }
}
