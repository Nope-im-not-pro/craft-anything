//! The Craft Anything desktop app.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use craft_anything_ai::AiHub;
use craft_anything_engine::{Engine, Session, Settings};
use craft_anything_ui_egui::CraftApp;
use std::path::PathBuf;
use std::sync::Arc;

#[cfg(any(windows, test))]
mod renderer;
mod start_guard;

/// Set when this start draws with the software renderer (WARP), as with
/// `CRAFT_ANYTHING_RENDERER=software`: the renderer chain chose it (see [`renderer`]), or the
/// environment picks the backend and the last start never opened its window.
#[cfg(windows)]
static RETRY_IN_SOFTWARE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

#[cfg(any(target_os = "linux", test))]
mod linux_gpu;

/// Opens the window on the engine and AI hub (a path on the command line opens as the first
/// target); if the window can't start, says why and where its log is.
fn main() -> eframe::Result {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "--version" || a == "-V") {
        println!("craft-anything {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }
    // A path on the command line (or from "Open with…") opens as the first target.
    let open: Option<PathBuf> = args.iter().find(|a| !a.starts_with('-')).map(PathBuf::from);
    // The window app has no console: what goes wrong before (or instead of) a window is kept in
    // desktop.log beside the settings, so a start that shows nothing leaves a trace.
    log_to_file();
    log::info!("starting {} (pid {})", env!("CARGO_PKG_VERSION"), std::process::id());
    // A driver that crashes natively leaves nothing behind but this mark (#74): the start after it
    // draws another way (on Windows, the next renderer, which is kept once it works).
    let config = craft_anything_engine::settings::config_dir();
    let last_failed = start_guard::last_start_failed(&config);
    if last_failed {
        log::warn!("the last start never opened its window");
    }
    // Windows: the way to draw this time, the next one when the last start failed (#794).
    #[cfg(windows)]
    let renderer = pick_renderer(&config, last_failed);
    start_guard::begin(&config);

    let settings = Settings::load();
    let mut session = Session::new(settings);
    let plugins = craft_anything_engine::plugins::install(&mut session);
    let mut layer_tools = craft_anything_ai::tools();
    layer_tools.extend(craft_anything_setup::tools::specs());
    let engine = Engine::shared_with(session, layer_tools, plugins);
    let hub = Arc::new(AiHub::new(engine.clone()));
    hub.install_key_store();
    log::info!("engine and AI ready; opening the window");

    let viewport = eframe::egui::ViewportBuilder::default()
        .with_title("Craft Anything")
        .with_app_id("ai.storyteller.craft-anything")
        .with_inner_size([1360.0, 860.0])
        .with_min_inner_size([760.0, 520.0])
        .with_resizable(true)
        .with_drag_and_drop(true);
    // Our own title bar draws the window controls, moves and resizes the window. macOS keeps its
    // own buttons and frame instead, with the content reaching under a see-through title bar (#20).
    let viewport = if cfg!(target_os = "macos") {
        viewport.with_fullsize_content_view(true).with_titlebar_shown(false).with_title_shown(false)
    } else {
        viewport.with_decorations(false)
    };
    // On macOS a window icon replaces the Dock icon, and the bundle's own icon is the one padded to
    // Apple's icon grid (the full tile looked bigger than other apps', #102). An empty icon keeps
    // the bundle's; no icon at all would bring eframe's own.
    let icon = if cfg!(target_os = "macos") { None } else { eframe::icon_data::from_png_bytes(craft_anything_ui_egui::APP_ICON_PNG).ok() };
    let viewport = viewport.with_icon(Arc::new(icon.unwrap_or_default()));
    #[cfg_attr(not(any(windows, target_os = "linux")), expect(unused_mut, reason = "only Windows and Linux pick their own graphics adapter"))]
    let mut options = eframe::NativeOptions { viewport, ..Default::default() };
    #[cfg(target_os = "linux")]
    if let eframe::egui_wgpu::WgpuSetup::CreateNew(setup) = &mut options.wgpu_options.wgpu_setup {
        // egui-wgpu's default is the discrete GPU; on a laptop whose integrated GPU drives the
        // panel, the window took 20 s to open and the whole desktop lagged (#571). Draw on the
        // GPU that drives the screen; `WGPU_ADAPTER_NAME` and `WGPU_POWER_PREF` still decide.
        setup.native_adapter_selector = Some(Arc::new(linux_gpu::pick_adapter));
    }
    #[cfg(windows)]
    if let eframe::egui_wgpu::WgpuSetup::CreateNew(setup) = &mut options.wgpu_options.wgpu_setup {
        // Starting GL loads the system's OpenGL driver before any adapter is picked, and some
        // Intel drivers crash in it (ig9icd64.dll, #196), so the window never opens. Similarly,
        // starting Vulkan loads Intel's Vulkan driver (igvk64.dll, #394) and crashes before any
        // window. DX12 (with WARP) covers all Windows systems; `WGPU_BACKEND` still asks for others.
        setup.instance_descriptor.backends = windows_backends(eframe::wgpu::Backends::from_env(), renderer);
        setup.instance_descriptor.backend_options.dx12.shader_compiler = dx12_compiler(eframe::wgpu::Dx12Compiler::from_env());
        setup.native_adapter_selector = Some(Arc::new(pick_adapter));
    }
    // Set once the first frames have been shown. Before that, eframe 0.36's wgpu panics when it
    // can't configure the window's surface (no usable GPU, or no desktop: winget-pkgs validation
    // starts the app that way and rejected the panic, exit 101), instead of returning an error.
    let shown = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let shown_by_app = Arc::clone(&shown);
    // AssertUnwindSafe: after a caught panic nothing from this closure is used again. The event
    // loop isn't retried; the app only reports the failure (below) and exits.
    let run = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        eframe::run_native(
            "Craft Anything",
            options,
            Box::new(move |cc| {
                log::info!("window open");
                CraftApp::setup_context(&cc.egui_ctx);
                let mut app = CraftApp::new(engine, hub);
                // The window is built hidden and its first frame is drawn afterwards, where a graphics
                // driver may still crash: the mark stays until frames have been shown (#74).
                app.after_first_frames(move || {
                    start_guard::opened(&config);
                    shown_by_app.store(true, std::sync::atomic::Ordering::Relaxed);
                });
                app.load_prefs(cc.storage);
                app.install_menu_bar(&cc.egui_ctx);
                if let Some(p) = open {
                    app.open_path(p);
                }
                Ok(Box::new(app))
            }),
        )
    }));
    let run = start_result(run, shown.load(std::sync::atomic::Ordering::Relaxed));
    match &run {
        Ok(()) => log::info!("closed"),
        Err(e) => {
            log::error!("the window couldn't start: {e}");
            // A wgpu error before the window was shown, with another renderer still to try: start
            // again with it at once rather than ask the person to (#794). The chain ends, so this
            // can't loop.
            #[cfg(windows)]
            if !shown.load(std::sync::atomic::Ordering::Relaxed)
                && let Some(next) = renderer.and_then(|r| renderer::relaunch_with(r, &e.to_string()))
                && {
                    start_guard::release();
                    relaunch(&args)
                }
            {
                log::warn!("starting again with {}", next.title());
                return run;
            }
            // Without a window the app would just vanish (#166): say why, and where the log is.
            let log = craft_anything_engine::settings::config_dir().join("desktop.log");
            #[cfg(windows)]
            let retry = next_start_note(renderer, shown.load(std::sync::atomic::Ordering::Relaxed));
            #[cfg(not(windows))]
            let retry = String::new();
            let _ = rfd::MessageDialog::new()
                .set_level(rfd::MessageLevel::Error)
                .set_title("Craft Anything couldn't open its window")
                .set_description(format!(
                    "{e}\n\n{}{retry} What happened is written to {}; please attach it if you report this.",
                    graphics_advice(&e.to_string()),
                    log.display()
                ))
                .set_buttons(rfd::MessageButtons::Ok)
                .show();
        }
    }
    run
}

/// The DX12 shader compiler. wgpu's own choice (`Auto`) takes a `dxcompiler.dll` it finds on the
/// PATH, and one without its `dxil.dll` makes unsigned shaders: on some drivers (an RTX 5060 Ti,
/// #522) the device is lost the moment the window opens. FXC comes with Windows and needs no extra
/// files; `WGPU_DX12_COMPILER` still asks for another.
#[cfg_attr(all(not(windows), not(test)), expect(dead_code, reason = "only Windows draws with DX12"))]
fn dx12_compiler(from_env: Option<eframe::wgpu::Dx12Compiler>) -> eframe::wgpu::Dx12Compiler {
    from_env.unwrap_or(eframe::wgpu::Dx12Compiler::Fxc)
}

/// Whether the software renderer (WARP) is asked for: `CRAFT_ANYTHING_RENDERER=software`.
#[cfg_attr(all(not(windows), not(test)), expect(dead_code, reason = "only Windows picks its own graphics adapter"))]
fn wants_software(setting: Option<&str>) -> bool {
    setting.is_some_and(|s| s.trim().eq_ignore_ascii_case("software"))
}

/// What the error dialog says the next start will do. Only a start whose window was never shown
/// leaves its mark, so only then does the next start move on to another renderer; after the window
/// was shown the next start draws as this one did, and the dialog says nothing about it.
#[cfg(windows)]
fn next_start_note(renderer: Option<renderer::Renderer>, shown: bool) -> String {
    match renderer {
        Some(r) if !shown => format!(" The next start will draw with {}.", renderer::choose(Some(r), true).title()),
        _ => String::new(),
    }
}

/// The result of a run that may have panicked. A panic before the window's first frames were shown
/// becomes an error, which the start-failure dialog then reports: on eframe 0.36 that is how wgpu
/// reports a surface it can't configure (no usable GPU, or no desktop). It can also be any other bug
/// in the app's first moments, so the text says "internal error" and doesn't blame the graphics. A
/// panic after the window was up is resumed, ending the app as a panic always did.
fn start_result(run: std::thread::Result<eframe::Result>, shown: bool) -> eframe::Result {
    match run {
        Ok(run) => run,
        Err(payload) if !shown => {
            Err(eframe::Error::AppCreation(format!("internal error before the window was shown: {}", panic_text(payload.as_ref())).into()))
        }
        Err(payload) => std::panic::resume_unwind(payload),
    }
}

/// The message a panic carried (`panic!` with a literal or a formatted string), or a generic one.
fn panic_text(payload: &(dyn std::any::Any + Send)) -> String {
    match (payload.downcast_ref::<&str>(), payload.downcast_ref::<String>()) {
        (Some(text), _) => (*text).to_string(),
        (None, Some(text)) => text.clone(),
        (None, None) => "an unknown panic".to_string(),
    }
}

/// What to try when the window can't start. A lost graphics device on Windows gets the way to the
/// software renderer; on macOS that setting does nothing (only Windows picks its adapter), so it
/// gets the update advice and the report path. Anything else gets the usual driver advice.
fn graphics_advice(error: &str) -> &'static str {
    if error.to_lowercase().contains("device is lost") {
        if cfg!(target_os = "macos") {
            "The Mac's graphics stopped while the window opened. Install the latest macOS update, then try again. If it keeps happening, please report it with the log file (Craft Anything issue #742 covers the Metal shader case)."
        } else {
            "The graphics driver stopped while the window opened. Update it, then try again. If it keeps happening, set the environment variable CRAFT_ANYTHING_RENDERER=software to draw without the graphics card (slower), and start the app again."
        }
    } else {
        "Updating your graphics driver often fixes this."
    }
}

/// Windows: the adapter that draws the window. GL is the last resort, Windows' own software
/// renderer (WARP) included: in a VMware VM, GL is the way to the virtual GPU and drew a black
/// window (#27). The fastest device wins, and between same-class devices DX12 comes before Vulkan
/// (#33). `WGPU_BACKEND` still narrows the choice.
#[cfg(windows)]
fn pick_adapter(adapters: &[eframe::wgpu::Adapter], surface: Option<&eframe::wgpu::Surface<'_>>) -> Result<eframe::wgpu::Adapter, String> {
    let software = wants_software(std::env::var("CRAFT_ANYTHING_RENDERER").ok().as_deref()) || RETRY_IN_SOFTWARE.load(std::sync::atomic::Ordering::Relaxed);
    let picked = adapters.iter().filter(|a| surface.is_none_or(|s| a.is_surface_supported(s))).min_by_key(|a| {
        let info = a.get_info();
        // Asked for software: WARP first, and the usual order among the rest.
        (software && info.device_type != eframe::wgpu::DeviceType::Cpu, adapter_rank(info.backend, info.device_type))
    });
    match picked {
        Some(a) => {
            log::info!("drawing with {:?}", a.get_info());
            Ok(a.clone())
        }
        None => Err(format!("None of the {} graphics adapters found can draw this window.", adapters.len())),
    }
}

/// Windows: the graphics backends to start. `WGPU_BACKEND` decides when set; otherwise DX12
/// only (#196, #394). Vulkan and GL load third-party driver DLLs (igvk64.dll, ig9icd64.dll) before
/// an adapter is chosen, which crash on some Intel systems. DX12 (with WARP) covers all Windows
/// installations; `WGPU_BACKEND=vulkan` or `gl` still asks for them explicitly.
/// After a start that failed, the renderer chain may have moved on to Vulkan or GL (#794): it is
/// only reached once DX12 has failed.
#[cfg(windows)]
fn windows_backends(from_env: Option<eframe::wgpu::Backends>, renderer: Option<renderer::Renderer>) -> eframe::wgpu::Backends {
    from_env.or(renderer.map(renderer::Renderer::backends)).unwrap_or(eframe::wgpu::Backends::DX12)
}

/// Windows: the renderer for this start (see [`renderer`]), remembered before the window is asked
/// for, so a start that dies makes the next one try the next renderer. `None` when
/// `WGPU_BACKEND` or `CRAFT_ANYTHING_RENDERER` decides; then a failed start only brings WARP, as
/// before.
#[cfg(windows)]
fn pick_renderer(config: &std::path::Path, last_failed: bool) -> Option<renderer::Renderer> {
    use std::sync::atomic::Ordering;
    let set = |name: &str| std::env::var_os(name).is_some_and(|v| !v.is_empty());
    if set("WGPU_BACKEND") || set("CRAFT_ANYTHING_RENDERER") {
        if last_failed {
            log::warn!("drawing without the graphics card this time");
            RETRY_IN_SOFTWARE.store(true, Ordering::Relaxed);
        }
        return None;
    }
    let version = env!("CARGO_PKG_VERSION");
    let chosen = renderer::choose(renderer::remembered(config, version), last_failed);
    renderer::remember(config, version, chosen);
    RETRY_IN_SOFTWARE.store(chosen.software(), Ordering::Relaxed);
    if last_failed {
        log::warn!("drawing with {} ({}), after a start that never opened its window", chosen.title(), chosen.name());
    } else if chosen != renderer::Renderer::Dx12 {
        log::info!("drawing with {} ({}), which opened the window last time", chosen.title(), chosen.name());
    }
    Some(chosen)
}

/// Starts the app again with the same arguments; whether it started.
#[cfg(windows)]
fn relaunch(args: &[String]) -> bool {
    let started = std::env::current_exe()
        .map_err(|e| e.to_string())
        .and_then(|exe| craft_anything_platform::process::Launch::new(&exe).args(args).label("Craft Anything").start().map_err(|e| e.to_string()));
    if let Err(e) = &started {
        log::warn!("couldn't start again: {e}");
    }
    started.is_ok()
}

/// Lower is better: GL last, fastest device first, DX12 before Vulkan only between same-class devices
/// (#33).
#[cfg(windows)]
fn adapter_rank(backend: eframe::wgpu::Backend, device: eframe::wgpu::DeviceType) -> (bool, u8, u8) {
    use eframe::wgpu::DeviceType;
    let is_gl = backend == eframe::wgpu::Backend::Gl;
    let speed = match device {
        DeviceType::DiscreteGpu => 0,
        DeviceType::IntegratedGpu => 1,
        DeviceType::VirtualGpu => 2,
        DeviceType::Cpu => 3,
        DeviceType::Other => 4,
    };
    let dx12_before_vulkan = match backend {
        eframe::wgpu::Backend::Dx12 => 0,
        eframe::wgpu::Backend::Vulkan => 1,
        _ => 2,
    };
    (is_gl, speed, dx12_before_vulkan)
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    use eframe::wgpu::{Backend as B, DeviceType as T};

    /// #27: a VMware VM offers its virtual GPU over GL and WARP over DX12; GL drew a black window.
    /// #33: a start that shows nothing is looked at through `desktop.log`; between same-class
    /// devices DX12 comes before Vulkan, and GL stays the last resort.
    #[test]
    fn dx12_comes_before_vulkan_and_gl_is_the_last_resort() {
        let best = |c: &[(B, T)]| c.iter().enumerate().min_by_key(|(_, (b, t))| adapter_rank(*b, *t)).map(|(i, _)| i);
        assert_eq!(best(&[(B::Gl, T::Other), (B::Dx12, T::Cpu)]), Some(1), "VMware: WARP, not GL");
        assert_eq!(best(&[(B::Gl, T::IntegratedGpu), (B::Dx12, T::Cpu)]), Some(1));
        assert_eq!(best(&[(B::Vulkan, T::DiscreteGpu), (B::Dx12, T::IntegratedGpu)]), Some(0), "fastest GPU first");
        assert_eq!(best(&[(B::Dx12, T::IntegratedGpu), (B::Vulkan, T::IntegratedGpu)]), Some(0), "DX12 before Vulkan between same-class devices");
        assert_eq!(best(&[(B::Dx12, T::Cpu), (B::Vulkan, T::DiscreteGpu)]), Some(1), "WARP does not beat a real Vulkan device");
        assert_eq!(best(&[(B::Dx12, T::IntegratedGpu), (B::Vulkan, T::DiscreteGpu), (B::Dx12, T::Cpu)]), Some(1), "fastest GPU first");
        assert_eq!(best(&[(B::Gl, T::Other)]), Some(0), "GL when it's all there is");
    }

    /// #196: an Intel OpenGL driver crashed the app as wgpu started GL, before any window.
    /// #394: an Intel Vulkan driver (igvk64.dll) crashed the app as wgpu started Vulkan.
    #[test]
    fn gl_and_vulkan_are_only_started_when_asked_for() {
        use eframe::wgpu::Backends;
        use renderer::Renderer;
        assert_eq!(windows_backends(None, None), Backends::DX12);
        assert_eq!(windows_backends(None, Some(Renderer::Dx12)), Backends::DX12);
        assert_eq!(windows_backends(None, Some(Renderer::Warp)), Backends::DX12, "WARP is a DX12 adapter");
        assert_eq!(windows_backends(Some(Backends::GL), None), Backends::GL, "WGPU_BACKEND=gl still works");
        assert_eq!(windows_backends(Some(Backends::VULKAN), None), Backends::VULKAN, "WGPU_BACKEND=vulkan still works");
        // #794: once DX12 and WARP failed, the chain reaches them without the variable.
        assert_eq!(windows_backends(None, Some(Renderer::Vulkan)), Backends::VULKAN);
        assert_eq!(windows_backends(None, Some(Renderer::Gl)), Backends::GL);
        assert_eq!(windows_backends(Some(Backends::DX12), Some(Renderer::Gl)), Backends::DX12, "the variable wins");
    }
}

/// Kept small: started afresh once it passes this size.
const LOG_MAX: u64 = 512 * 1024;

/// Warnings and errors from everything, plus our own progress, appended to `desktop.log`; and
/// panics, which would otherwise vanish with the window.
fn log_to_file() {
    use std::io::Write as _;
    struct FileLog(craft_anything_platform::sync::Mutex<Option<std::fs::File>>);
    impl log::Log for FileLog {
        fn enabled(&self, m: &log::Metadata<'_>) -> bool {
            m.level() <= log::Level::Warn || (m.level() <= log::Level::Info && m.target().starts_with("craft_anything"))
        }
        fn log(&self, r: &log::Record<'_>) {
            if !self.enabled(r.metadata()) {
                return;
            }
            let at = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
            if let Ok(mut f) = self.0.lock()
                && let Some(f) = f.as_mut()
            {
                let _ = writeln!(f, "{at} [{}] {}: {}", r.level(), r.target(), r.args());
            }
        }
        fn flush(&self) {}
    }
    let dir = craft_anything_engine::settings::config_dir();
    let path = dir.join("desktop.log");
    let _ = std::fs::create_dir_all(&dir);
    let fresh = std::fs::metadata(&path).is_ok_and(|m| m.len() > LOG_MAX);
    let file = std::fs::OpenOptions::new().create(true).append(!fresh).write(true).truncate(fresh).open(&path).ok();
    if log::set_logger(Box::leak(Box::new(FileLog(craft_anything_platform::sync::Mutex::new(file))))).is_ok() {
        log::set_max_level(log::LevelFilter::Info);
    }
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        log::error!("panic: {info}");
        default_hook(info);
    }));
    craft_anything_platform::sync::start_watchdog(craft_anything_engine::settings::config_dir().join("logs"));
}

#[cfg(test)]
mod graphics_tests {
    use super::*;
    use eframe::wgpu::Dx12Compiler;

    /// A panic before the window showed becomes the error the start-failure dialog shows: wgpu's
    /// own message when there is one (`panic!` with a literal or a formatted string).
    #[test]
    fn a_start_panic_keeps_its_message() {
        let literal: Box<dyn std::any::Any + Send> = Box::new("wgpu error: Validation Error");
        let formatted: Box<dyn std::any::Any + Send> = Box::new(String::from("In Surface::configure: Invalid surface"));
        let other: Box<dyn std::any::Any + Send> = Box::new(7_u8);
        assert_eq!(panic_text(literal.as_ref()), "wgpu error: Validation Error");
        assert_eq!(panic_text(formatted.as_ref()), "In Surface::configure: Invalid surface");
        assert_eq!(panic_text(other.as_ref()), "an unknown panic");
    }

    /// A run that ended normally, or with an error, comes back as it was.
    #[test]
    fn a_run_without_a_panic_is_passed_through() {
        assert!(start_result(Ok(Ok(())), false).is_ok());
        let error = start_result(Ok(Err(eframe::Error::AppCreation("no adapter".into()))), true);
        assert!(matches!(error, Err(eframe::Error::AppCreation(e)) if e.to_string() == "no adapter"));
    }

    /// A panic before the window showed becomes the start error the dialog reports, with the
    /// panic's message and without blaming the graphics.
    #[test]
    fn a_panic_before_the_window_showed_becomes_a_start_error() {
        let run: std::thread::Result<eframe::Result> = Err(Box::new("In Surface::configure: Invalid surface"));
        let Err(eframe::Error::AppCreation(e)) = start_result(run, false) else {
            panic!("expected a start error");
        };
        assert_eq!(e.to_string(), "internal error before the window was shown: In Surface::configure: Invalid surface");
    }

    /// A panic after the window was up isn't turned into a start error: it is resumed, with its
    /// own payload, as if nothing had caught it.
    #[test]
    fn a_panic_after_the_window_showed_is_resumed() {
        let resumed = std::panic::catch_unwind(|| {
            let run: std::thread::Result<eframe::Result> = Err(Box::new("a bug in a frame"));
            let _ = start_result(run, true);
        });
        let payload = resumed.expect_err("the panic must be resumed");
        assert_eq!(panic_text(payload.as_ref()), "a bug in a frame");
    }

    /// #522: wgpu's `Auto` found a `dxcompiler.dll` without `dxil.dll`, and the device was lost on
    /// an RTX 5060 Ti. FXC is the default; the environment variable still decides when set.
    #[test]
    fn dx12_draws_with_the_compiler_windows_ships_unless_asked_otherwise() {
        assert!(matches!(dx12_compiler(None), Dx12Compiler::Fxc));
        assert!(matches!(dx12_compiler(Some(Dx12Compiler::StaticDxc)), Dx12Compiler::StaticDxc));
    }

    #[cfg(windows)]
    #[test]
    fn a_lost_device_says_how_to_draw_without_the_graphics_card() {
        assert!(graphics_advice("WGPU error: Parent device is lost").contains("CRAFT_ANYTHING_RENDERER=software"));
        assert!(!graphics_advice("No adapter").contains("software"));
        assert!(wants_software(Some(" Software ")) && !wants_software(Some("gpu")) && !wants_software(None));
    }

    /// #742: on macOS `CRAFT_ANYTHING_RENDERER` does nothing, so the advice must not name it.
    #[cfg(target_os = "macos")]
    #[test]
    fn a_lost_device_on_a_mac_does_not_point_at_the_windows_only_setting() {
        let advice = graphics_advice("WGPU error: Parent device is lost");
        assert!(!advice.contains("CRAFT_ANYTHING_RENDERER"), "{advice}");
        assert!(advice.contains("#742"));
        assert!(!graphics_advice("No adapter").contains("CRAFT_ANYTHING_RENDERER"));
    }

    #[cfg(not(windows))]
    #[test]
    fn wants_software_reads_the_setting() {
        assert!(wants_software(Some(" Software ")) && !wants_software(Some("gpu")) && !wants_software(None));
    }

    /// #909 review: only a start whose window never showed moves on to another renderer, so only
    /// then does the dialog say the next start will draw differently.
    #[cfg(windows)]
    #[test]
    fn the_dialog_promises_another_renderer_only_before_the_window_was_shown() {
        use renderer::Renderer;
        assert_eq!(next_start_note(Some(Renderer::Dx12), false), " The next start will draw with Windows' software renderer.");
        assert_eq!(next_start_note(Some(Renderer::Dx12), true), "");
        assert_eq!(next_start_note(None, false), "", "a renderer from the environment is never changed");
    }
}
