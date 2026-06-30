// hardware init, construct Kernel + AppManager, boot, run

extern crate alloc;

use crate::rustmix_x4::apps::home::HomeApp;
use crate::rustmix_x4::apps::manager::AppManager;
use crate::rustmix_x4::x4_apps::apps::Launcher;
use crate::rustmix_x4::x4_apps::apps::files::FilesApp;
use crate::rustmix_x4::x4_apps::apps::reader::ReaderApp;
use crate::rustmix_x4::x4_apps::apps::settings::SettingsApp;
use crate::rustmix_x4::x4_apps::apps::widgets::{ButtonFeedback, QuickMenu};
use crate::rustmix_x4::x4_apps::ui::paint_stack;
use crate::rustmix_x4::x4_kernel::app::AppShell;
use crate::rustmix_x4::x4_kernel::board::{Board, speed_up_spi};
use crate::rustmix_x4::x4_kernel::drivers::battery;
use crate::rustmix_x4::x4_kernel::drivers::input::InputDriver;
use crate::rustmix_x4::x4_kernel::drivers::sdcard::SdStorage;
use crate::rustmix_x4::x4_kernel::drivers::storage;
use crate::rustmix_x4::x4_kernel::drivers::strip::StripBuffer;
use crate::rustmix_x4::x4_kernel::kernel::BookmarkCache;
use crate::rustmix_x4::x4_kernel::kernel::BootConsole;
use crate::rustmix_x4::x4_kernel::kernel::Kernel;
use crate::rustmix_x4::x4_kernel::kernel::dir_cache::DirCache;
use crate::rustmix_x4::x4_kernel::kernel::tasks;
use crate::rustmix_x4::x4_kernel::kernel::work_queue;
use esp_backtrace as _;
use esp_hal::clock::CpuClock;
use esp_hal::delay::Delay;
use esp_hal::interrupt::software::SoftwareInterruptControl;
use esp_hal::ram;
use esp_hal::timer::timg::TimerGroup;
use log::info;
use static_cell::{ConstStaticCell, StaticCell};

use crate::rustmix_x4::display::redraw_policy_runtime::RustmixRedrawPolicyAppLayer;

esp_bootloader_esp_idf::esp_app_desc!();

// heavy statics: kept out of the async future to keep it ~200 B
static STRIP: ConstStaticCell<StripBuffer> = ConstStaticCell::new(StripBuffer::new());
static READER: ConstStaticCell<ReaderApp> = ConstStaticCell::new(ReaderApp::new());
static LAUNCHER: ConstStaticCell<Launcher> = ConstStaticCell::new(Launcher::new());
static QUICK_MENU: ConstStaticCell<QuickMenu> = ConstStaticCell::new(QuickMenu::new());
static BUMPS: ConstStaticCell<ButtonFeedback> = ConstStaticCell::new(ButtonFeedback::new());
static DIR_CACHE: ConstStaticCell<DirCache> = ConstStaticCell::new(DirCache::new());
static BM_CACHE: ConstStaticCell<BookmarkCache> = ConstStaticCell::new(BookmarkCache::new());

// BootConsole is heap-allocated during boot and dropped after display,
// reclaiming ~3 KB that would otherwise sit unused in .bss forever.
static HOME: StaticCell<HomeApp> = StaticCell::new();
static FILES: StaticCell<FilesApp> = StaticCell::new();
static SETTINGS: StaticCell<SettingsApp> = StaticCell::new();

#[esp_rtos::main]
async fn main(spawner: embassy_executor::Spawner) -> ! {
    crate::rustmix_x4::boot::RustmixBoot::emit_runtime_ready_marker();
    esp_println::println!(
        "{}",
        crate::rustmix_x4::network::RustmixWifiRuntimeOwnership::marker()
    );

    #[cfg(all(target_arch = "riscv32", feature = "r10-ble-host"))]
    {
        if option_env!("R10_BLE_X4_PROBE_BRIDGE") == Some("1") {
            crate::rustmix_x4::ring_remote::r10_ble_x4_emit_probe_only_bridge_startup_log();
        }
    }

    #[cfg(all(target_arch = "riscv32", feature = "r10-ble-host"))]
    {
        if option_env!("R10_BLE_X4_BACKEND_TASK") == Some("1") {
            crate::rustmix_x4::ring_remote::r10_ble_x4_emit_esp32c3_probe_backend_startup_log();
        }
    }

    #[cfg(all(target_arch = "riscv32", feature = "r10-ble-host"))]
    {
        if option_env!("R10_BLE_X4_OPERATION_QUEUE") == Some("1") {
            crate::rustmix_x4::ring_remote::r10_ble_x4_emit_esp32c3_probe_operation_queue_startup_log();
        }
    }

    #[cfg(all(target_arch = "riscv32", feature = "r10-ble-host"))]
    {
        if option_env!("R10_BLE_X4_RUNNER") == Some("1") {
            crate::rustmix_x4::ring_remote::r10_ble_x4_emit_esp32c3_probe_runner_startup_log();
        }
    }

    #[cfg(all(target_arch = "riscv32", feature = "r10-ble-host"))]
    {
        if option_env!("R10_BLE_X4_READER_REMOTE_GUARD") == Some("1") {
            crate::rustmix_x4::ring_remote::r10_ble_x4_emit_reader_remote_guard_startup_log();
        }
    }

    #[cfg(all(target_arch = "riscv32", feature = "r10-ble-host"))]
    {
        if option_env!("R10_BLE_X4_SETTINGS") == Some("1") {
            crate::rustmix_x4::ring_remote::r10_ble_x4_emit_settings_status_startup_log();
        }
    }
    let _ = crate::rustmix_x4::contracts::storage_path_helpers::RustmixStoragePathHelpers::active_runtime_adoption_probe();
    let _ = crate::rustmix_x4::contracts::input_semantics::RustmixInputSemantics::active_runtime_adoption_probe();
    let _ = crate::rustmix_x4::input::input_semantics_runtime::RustmixInputSemanticsRuntimeBridge::active_runtime_preflight();
    let _ = crate::rustmix_x4::input::active_semantic_mapper::RustmixActiveInputSemanticMapper::active_runtime_preflight();
    let _ = crate::rustmix_x4::input::input_adc_runtime::RustmixInputAdcRuntimeBridge::active_runtime_preflight();
    let _ = crate::rustmix_x4::contracts::display_geometry::RustmixDisplayGeometry::active_runtime_adoption_probe();
    let _ = crate::rustmix_x4::display::display_geometry_runtime::RustmixDisplayGeometryRuntimeBridge::active_runtime_preflight();
    let _ = crate::rustmix_x4::physical::spi_bus_runtime::RustmixSpiBusRuntimeBridge::active_runtime_preflight();
    let _ = crate::rustmix_x4::io::storage_state_runtime::RustmixStorageStateRuntimeBridge::active_runtime_preflight();
    esp_println::logger::init_logger_from_env();

    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let peripherals = esp_hal::init(config);

    paint_stack();

    // 108 KB main DRAM heap; leaves ~56 KB for stack
    esp_alloc::heap_allocator!(size: 110_592);

    // reclaim ~64 KB from 2nd-stage bootloader; net heap ~172 KB
    esp_alloc::heap_allocator!(#[ram(reclaimed)] size: 64_000);

    let _ = crate::rustmix_x4::io::storage_state_runtime::RustmixStorageStateRuntimeBridge::active_runtime_alloc_preflight();

    let app_shell = AppShell::new();
    info!("app shell initialised: {:?}", app_shell.screen());

    let mut console = alloc::boxed::Box::new(BootConsole::new());
    console.push("rustmix-os 0.1.0");
    console.push("esp32c3 rv32imc 160mhz");
    console.push("heap: 172K (108K + 64K reclaimed)");
    console.push("app shell: home/browser/reader scaffold");

    info!("booting...");

    // Safety: TIMG0 and SW_INTERRUPT are cloned here and consumed by
    // esp_rtos::start. They are never used again after this point.
    //
    // Board::init (which takes ownership of `peripherals`) does not
    // touch TIMG0 or SW_INTERRUPT, see the pin ownership table in
    // board/mod.rs for the full split.
    let timg0 = TimerGroup::new(unsafe { peripherals.TIMG0.clone_unchecked() });
    let sw_ints =
        SoftwareInterruptControl::new(unsafe { peripherals.SW_INTERRUPT.clone_unchecked() });
    esp_rtos::start(timg0.timer0, sw_ints.software_interrupt0);

    // Peripherals move into Board::init, which splits them across
    // init_input (ADC pins, GPIO3, IO_MUX) and init_spi_peripherals
    // (SPI2, DMA, display + SD GPIOs). Each peripheral is used in
    // exactly one place, see the ownership table in board/mod.rs.
    let board = Board::init(peripherals);
    console.push("spi: dma ch0, 4096B tx+rx");

    let mut epd = board.display.epd;
    let mut delay = Delay::new();
    epd.init(&mut delay);
    console.push("epd: ssd1677 800x480 init");

    speed_up_spi();
    console.push("spi: 400kHz -> 20MHz");
    let sd = match board.storage.sd_card {
        Some(card) => {
            console.push("sd: card detected");
            SdStorage::mount(card).await
        }
        None => {
            console.push("sd: not found");
            SdStorage::empty()
        }
    };

    let sd_ok = sd.probe_ok();
    if sd_ok {
        console.push("sd: fat32 mounted");
        if let Err(e) = storage::ensure_x4_dir_async(&sd).await {
            console.push("sd: rustmix dir failed");
            log::warn!("ensure_RUSTMIX_DIR: {:?}", e);
        } else if let Err(e) = storage::provision_rustmix_sd_defaults(&sd) {
            console.push("sd: rustmix defaults skipped");
            log::warn!("provision_RUSTMIX_defaults: {:?}", e);
        } else {
            console.push("sd: /RUSTMIX ready");
        }
    }

    let mut input = InputDriver::new(board.input);
    let battery_mv = battery::adc_to_battery_mv(input.read_battery_mv());

    let mut kernel = Kernel::new(
        sd,
        epd,
        STRIP.take(),
        DIR_CACHE.take(),
        BM_CACHE.take(),
        delay,
        sd_ok,
        battery_mv,
    );

    let app_mgr = AppManager::new(
        LAUNCHER.take(),
        HOME.init(HomeApp::new()),
        FILES.init(FilesApp::new()),
        READER.take(),
        SETTINGS.init(SettingsApp::new()),
        app_shell,
        QUICK_MENU.take(),
        BUMPS.take(),
        crate::rustmix_x4::input::active_semantic_mapper::RustmixActiveInputSemanticMapper::new_imported_button_mapper(),
    );

    info!(
        "app shell bound into app manager: {:?}",
        app_mgr.app_shell().screen()
    );

    let mut app_layer = RustmixRedrawPolicyAppLayer::new(app_mgr);
    console.push(RustmixRedrawPolicyAppLayer::MARKER);

    console.push("kernel: constructed");

    kernel.show_boot_console(&console).await;
    drop(console); // reclaim ~3 KB of heap

    kernel.boot(&mut app_layer).await;

    // register the image decoder so the kernel's worker task can
    // decode JPEG/PNG without depending on smol-epub directly
    work_queue::register_image_decoder(|data, is_jpeg, max_w, max_h| {
        let raw = if is_jpeg {
            smol_epub::jpeg::decode_jpeg_fit(data, max_w, max_h)
        } else {
            smol_epub::png::decode_png_fit(data, max_w, max_h)
        };

        raw.map(|img| work_queue::DecodedImage {
            width: img.width,
            height: img.height,
            data: img.data,
            stride: img.stride,
        })
    });

    spawner
        .spawn(tasks::input_task(input))
        .expect("spawn input_task");
    spawner
        .spawn(tasks::housekeeping_task())
        .expect("spawn housekeeping_task");
    spawner
        .spawn(tasks::idle_timeout_task())
        .expect("spawn idle_timeout_task");
    spawner
        .spawn(work_queue::worker_task())
        .expect("spawn worker_task");

    info!("kernel ready.");
    kernel.run(&mut app_layer).await
}
