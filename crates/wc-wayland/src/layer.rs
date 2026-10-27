//! Runs one egui UI on a wlr-layer-shell surface: the desktop (background layer, whole
//! output) and the taskbar (top layer, bottom edge, reserving its height).
//!
//! winit can only create ordinary windows, and on Wayland an ordinary window cannot place
//! itself; layer surfaces are how shells put panels and backgrounds where they belong.
//! Rendering is the same as eframe's: egui_glow on an EGL window surface (Mesa).
use egui_glow::glow::{self, HasContext as _};
use glutin::{
    config::{Config, ConfigTemplateBuilder, GlConfig as _},
    context::{ContextApi, ContextAttributesBuilder, PossiblyCurrentContext, Version},
    display::{Display, DisplayApiPreference},
    prelude::*,
    surface::{Surface, SurfaceAttributesBuilder, SwapInterval, WindowSurface},
};
use raw_window_handle::{
    RawDisplayHandle, RawWindowHandle, WaylandDisplayHandle, WaylandWindowHandle,
};
use smithay_client_toolkit::{
    compositor::{CompositorHandler, CompositorState},
    delegate_compositor, delegate_keyboard, delegate_layer, delegate_output, delegate_pointer,
    delegate_registry, delegate_seat, delegate_shm,
    output::{OutputHandler, OutputState},
    reexports::{
        calloop::{ping::make_ping, EventLoop},
        calloop_wayland_source::WaylandSource,
        client::{
            globals::registry_queue_init,
            protocol::{wl_keyboard, wl_output, wl_pointer, wl_seat, wl_surface},
            Connection, Proxy, QueueHandle,
        },
    },
    registry::{ProvidesRegistryState, RegistryState},
    registry_handlers,
    seat::{
        keyboard::{KeyEvent, KeyboardHandler, Keysym, Modifiers, RawModifiers},
        pointer::{
            CursorIcon, PointerEvent, PointerEventKind, PointerHandler, ThemeSpec, ThemedPointer,
        },
        Capability, SeatHandler, SeatState,
    },
    shell::{
        wlr_layer::{LayerShell, LayerShellHandler, LayerSurface, LayerSurfaceConfigure},
        WaylandSurface,
    },
    shm::{Shm, ShmHandler},
};
use std::{
    ffi::c_void,
    num::NonZeroU32,
    ptr::NonNull,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

pub use smithay_client_toolkit::shell::wlr_layer::{Anchor, KeyboardInteractivity, Layer};

/// Where and how the surface is placed.
#[derive(Clone, Copy)]
pub struct Options {
    /// Layer-shell namespace; compositors use it in rules ("wincraft-desktop").
    pub namespace: &'static str,
    pub layer: Layer,
    pub anchor: Anchor,
    /// 0 in a direction anchored on both sides means "the whole output".
    pub size: (u32, u32),
    /// Pixels reserved at the anchored edge (the taskbar), -1 to ignore others' (the desktop).
    pub exclusive_zone: i32,
    pub keyboard: KeyboardInteractivity,
}

/// A shell UI drawn on a layer surface. `ViewportCommand::Close` ends [`run`].
pub trait ShellApp {
    fn update(&mut self, ctx: &egui::Context);
}

/// The GL context outlives layer surfaces: when the compositor closes one (no output yet,
/// monitor unplugged) only the EGL window surface is made again, so egui's textures stay.
struct Gl {
    display: Display,
    config: Config,
    surface: Option<Surface<WindowSurface>>,
    context: PossiblyCurrentContext,
    gl: Arc<glow::Context>,
    painter: egui_glow::Painter,
}

struct Runner {
    conn: Connection,
    registry: RegistryState,
    seat_state: SeatState,
    output_state: OutputState,
    compositor: CompositorState,
    shm: Shm,
    qh: QueueHandle<Runner>,
    layer_shell: LayerShell,
    options: Options,
    layer: LayerSurface,
    /// When a closed surface is made again, and how often that happened in a row.
    recreate_at: Option<Instant>,
    closures: u32,
    app: Box<dyn ShellApp>,
    ctx: egui::Context,
    gl: Option<Gl>,
    size: (u32, u32),
    started: Instant,
    /// Repaints asked for by egui (also from other threads, through `requested`).
    next_frame: Option<Instant>,
    requested: Arc<Mutex<Option<Instant>>>,
    events: Vec<egui::Event>,
    modifiers: egui::Modifiers,
    pointer_pos: Option<egui::Pos2>,
    focused: bool,
    keyboard: Option<wl_keyboard::WlKeyboard>,
    pointer: Option<ThemedPointer>,
    cursor: egui::CursorIcon,
    exit: Option<Result<(), String>>,
}

pub fn run(
    options: Options,
    make: impl FnOnce(&egui::Context) -> Box<dyn ShellApp>,
) -> Result<(), String> {
    let conn = Connection::connect_to_env().map_err(|e| format!("no Wayland compositor: {e}"))?;
    let (globals, queue) = registry_queue_init::<Runner>(&conn).map_err(|e| e.to_string())?;
    let qh = queue.handle();
    let mut event_loop: EventLoop<'static, Runner> =
        EventLoop::try_new().map_err(|e| e.to_string())?;
    let loop_handle = event_loop.handle();
    WaylandSource::new(conn.clone(), queue)
        .insert(loop_handle.clone())
        .map_err(|e| e.to_string())?;

    let compositor =
        CompositorState::bind(&globals, &qh).map_err(|e| format!("wl_compositor: {e}"))?;
    let layer_shell = LayerShell::bind(&globals, &qh)
        .map_err(|e| format!("the compositor has no layer shell: {e}"))?;
    let shm = Shm::bind(&globals, &qh).map_err(|e| format!("wl_shm: {e}"))?;
    let layer = create_layer(&layer_shell, &compositor, &qh, &options);

    let ctx = egui::Context::default();
    let requested = Arc::new(Mutex::new(None::<Instant>));
    let (ping, ping_source) = make_ping().map_err(|e| e.to_string())?;
    {
        let requested = requested.clone();
        ctx.set_request_repaint_callback(move |info| {
            let at = Instant::now() + info.delay;
            let mut next = requested.lock().unwrap();
            *next = Some(next.map_or(at, |t| t.min(at)));
            ping.ping();
        });
    }
    loop_handle
        .insert_source(ping_source, |_, _, _| {})
        .map_err(|e| e.to_string())?;
    let app = make(&ctx);
    let mut runner = Runner {
        conn,
        registry: RegistryState::new(&globals),
        seat_state: SeatState::new(&globals, &qh),
        output_state: OutputState::new(&globals, &qh),
        compositor,
        shm,
        qh,
        layer_shell,
        options,
        layer,
        recreate_at: None,
        closures: 0,
        app,
        ctx,
        gl: None,
        size: (0, 0),
        started: Instant::now(),
        next_frame: None,
        requested,
        events: vec![],
        modifiers: egui::Modifiers::default(),
        pointer_pos: None,
        focused: false,
        keyboard: None,
        pointer: None,
        cursor: egui::CursorIcon::Default,
        exit: None,
    };
    loop {
        let now = Instant::now();
        let due = runner.due();
        let timeout = due.map_or(Duration::from_secs(1), |t| t.saturating_duration_since(now));
        event_loop
            .dispatch(Some(timeout), &mut runner)
            .map_err(|e| e.to_string())?;
        if let Some(result) = runner.exit.take() {
            return result;
        }
        if runner.recreate_at.is_some_and(|t| t <= Instant::now()) {
            runner.recreate_at = None;
            runner.size = (0, 0);
            runner.layer = create_layer(
                &runner.layer_shell,
                &runner.compositor,
                &runner.qh,
                &runner.options,
            );
        }
        let drawable = runner.gl.as_ref().is_some_and(|gl| gl.surface.is_some());
        if drawable && runner.due().is_some_and(|t| t <= Instant::now()) {
            if let Err(e) = runner.draw() {
                return Err(e);
            }
        }
    }
}

fn create_layer(
    shell: &LayerShell,
    compositor: &CompositorState,
    qh: &QueueHandle<Runner>,
    options: &Options,
) -> LayerSurface {
    let layer = shell.create_layer_surface(
        qh,
        compositor.create_surface(qh),
        options.layer,
        Some(options.namespace),
        None,
    );
    layer.set_anchor(options.anchor);
    layer.set_size(options.size.0, options.size.1);
    layer.set_exclusive_zone(options.exclusive_zone);
    layer.set_keyboard_interactivity(options.keyboard);
    // The first commit carries no buffer; the compositor answers with a configure.
    layer.commit();
    layer
}

impl Runner {
    fn due(&self) -> Option<Instant> {
        if self.recreate_at.is_some() {
            return self.recreate_at;
        }
        let requested = *self.requested.lock().unwrap();
        match (self.next_frame, requested) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, b) => a.or(b),
        }
    }

    fn wake(&mut self) {
        self.next_frame = Some(Instant::now());
    }

    fn init_gl(&mut self) -> Result<Gl, String> {
        let display_ptr = NonNull::new(self.conn.backend().display_ptr() as *mut c_void)
            .ok_or("no wl_display")?;
        let raw_display = RawDisplayHandle::Wayland(WaylandDisplayHandle::new(display_ptr));
        // SAFETY: the display pointer stays valid while `conn` lives, which outlives `Gl`.
        let display = unsafe { Display::new(raw_display, DisplayApiPreference::Egl) }
            .map_err(|e| format!("EGL: {e}"))?;
        let template = ConfigTemplateBuilder::new().with_alpha_size(8).build();
        // SAFETY: plain EGL config query on a valid display.
        let config = unsafe { display.find_configs(template) }
            .map_err(|e| format!("EGL configs: {e}"))?
            .min_by_key(|c| c.num_samples())
            .ok_or("no EGL config")?;
        let (raw_window, surface) = self.window_surface(&display, &config)?;
        let apis = [
            ContextApi::OpenGl(None),
            ContextApi::Gles(Some(Version::new(3, 0))),
            ContextApi::Gles(Some(Version::new(2, 0))),
        ];
        let context = apis
            .into_iter()
            // SAFETY: context creation on a valid display and config.
            .find_map(|api| {
                unsafe {
                    display.create_context(
                        &config,
                        &ContextAttributesBuilder::new()
                            .with_context_api(api)
                            .build(Some(raw_window)),
                    )
                }
                .ok()
            })
            .ok_or("no OpenGL context")?
            .make_current(&surface)
            .map_err(|e| format!("make_current: {e}"))?;
        // A covered surface (the desktop behind maximized windows) gets no frame callbacks;
        // a blocking swap would freeze the shell host, so never wait for vsync.
        let _ = surface.set_swap_interval(&context, SwapInterval::DontWait);
        // SAFETY: the loader returns functions of the current context.
        let gl = Arc::new(unsafe {
            glow::Context::from_loader_function_cstr(|s| display.get_proc_address(s))
        });
        let painter = egui_glow::Painter::new(gl.clone(), "", None, false)
            .map_err(|e| format!("egui painter: {e}"))?;
        Ok(Gl {
            display,
            config,
            surface: Some(surface),
            context,
            gl,
            painter,
        })
    }

    /// An EGL window surface for the current layer surface, at the configured size.
    fn window_surface(
        &self,
        display: &Display,
        config: &Config,
    ) -> Result<(RawWindowHandle, Surface<WindowSurface>), String> {
        let (w, h) = (
            NonZeroU32::new(self.size.0).unwrap_or(NonZeroU32::MIN),
            NonZeroU32::new(self.size.1).unwrap_or(NonZeroU32::MIN),
        );
        let surface_ptr = NonNull::new(self.layer.wl_surface().id().as_ptr() as *mut c_void)
            .ok_or("no wl_surface")?;
        let raw_window = RawWindowHandle::Wayland(WaylandWindowHandle::new(surface_ptr));
        let attributes = SurfaceAttributesBuilder::<WindowSurface>::new().build(raw_window, w, h);
        // SAFETY: the wl_surface lives as long as the layer surface owned by this runner;
        // the EGL surface is dropped first when the layer surface is replaced.
        let surface = unsafe { display.create_window_surface(config, &attributes) }
            .map_err(|e| format!("EGL surface: {e}"))?;
        Ok((raw_window, surface))
    }

    /// Binds the context to a new layer surface after the previous one was closed.
    fn attach(&mut self) -> Result<(), String> {
        let Some(mut gl) = self.gl.take() else {
            return Ok(());
        };
        let outcome = self
            .window_surface(&gl.display, &gl.config)
            .and_then(|(_, surface)| {
                gl.context
                    .make_current(&surface)
                    .map_err(|e| format!("make_current: {e}"))?;
                let _ = surface.set_swap_interval(&gl.context, SwapInterval::DontWait);
                gl.surface = Some(surface);
                Ok(())
            });
        self.gl = Some(gl);
        outcome
    }

    fn draw(&mut self) -> Result<(), String> {
        self.next_frame = None;
        *self.requested.lock().unwrap() = None;
        let (w, h) = (self.size.0 as f32, self.size.1 as f32);
        let mut raw = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(w, h),
            )),
            time: Some(self.started.elapsed().as_secs_f64()),
            modifiers: self.modifiers,
            events: std::mem::take(&mut self.events),
            focused: self.focused,
            ..Default::default()
        };
        let viewport = raw.viewports.entry(egui::ViewportId::ROOT).or_default();
        viewport.native_pixels_per_point = Some(1.0);
        viewport.focused = Some(self.focused);
        viewport.inner_rect = raw.screen_rect;
        let app = &mut self.app;
        let output = self.ctx.run(raw, |ctx| app.update(ctx));
        let close = output
            .viewport_output
            .get(&egui::ViewportId::ROOT)
            .is_some_and(|v| {
                v.commands
                    .iter()
                    .any(|c| matches!(c, egui::ViewportCommand::Close))
            });
        if let Some(delay) = output
            .viewport_output
            .get(&egui::ViewportId::ROOT)
            .map(|v| v.repaint_delay)
        {
            if delay < Duration::from_secs(3600) {
                let at = Instant::now() + delay;
                self.next_frame = Some(self.next_frame.map_or(at, |t| t.min(at)));
            }
        }
        if output.platform_output.cursor_icon != self.cursor {
            self.cursor = output.platform_output.cursor_icon;
            self.apply_cursor();
        }
        let primitives = self.ctx.tessellate(output.shapes, output.pixels_per_point);
        let size = self.size;
        let gl = self.gl.as_mut().ok_or("not configured")?;
        // SAFETY: plain GL state calls on the current context.
        unsafe {
            gl.gl.viewport(0, 0, size.0 as i32, size.1 as i32);
            gl.gl.clear_color(0.0, 0.0, 0.0, 0.0);
            gl.gl.clear(glow::COLOR_BUFFER_BIT);
        }
        gl.painter.paint_and_update_textures(
            [size.0, size.1],
            output.pixels_per_point,
            &primitives,
            &output.textures_delta,
        );
        if let Some(surface) = &gl.surface {
            surface
                .swap_buffers(&gl.context)
                .map_err(|e| format!("swap: {e}"))?;
        }
        self.closures = 0;
        if close {
            self.exit = Some(Ok(()));
        }
        Ok(())
    }

    fn apply_cursor(&self) {
        let Some(pointer) = &self.pointer else { return };
        use egui::CursorIcon as E;
        let icon = match self.cursor {
            E::None => {
                let _ = pointer.hide_cursor();
                return;
            }
            E::PointingHand => CursorIcon::Pointer,
            E::Text | E::VerticalText => CursorIcon::Text,
            E::ResizeHorizontal | E::ResizeEast | E::ResizeWest | E::ResizeColumn => {
                CursorIcon::EwResize
            }
            E::ResizeVertical | E::ResizeNorth | E::ResizeSouth | E::ResizeRow => {
                CursorIcon::NsResize
            }
            E::Grab => CursorIcon::Grab,
            E::Grabbing => CursorIcon::Grabbing,
            E::Move | E::AllScroll => CursorIcon::Move,
            E::NotAllowed | E::NoDrop => CursorIcon::NotAllowed,
            E::Wait => CursorIcon::Wait,
            E::Progress => CursorIcon::Progress,
            E::Help => CursorIcon::Help,
            E::Crosshair => CursorIcon::Crosshair,
            _ => CursorIcon::Default,
        };
        let _ = pointer.set_cursor(&self.conn, icon);
    }

    fn key(&mut self, event: KeyEvent, pressed: bool, repeat: bool) {
        if let Some(key) = egui_key(event.keysym) {
            self.events.push(egui::Event::Key {
                key,
                physical_key: None,
                pressed,
                repeat,
                modifiers: self.modifiers,
            });
        }
        let typing = !self.modifiers.ctrl && !self.modifiers.alt && !self.modifiers.mac_cmd;
        if pressed && typing {
            if let Some(text) = event
                .utf8
                .filter(|t| !t.is_empty() && t.chars().all(|c| !c.is_control()))
            {
                self.events.push(egui::Event::Text(text));
            }
        }
        self.wake();
    }
}

fn egui_key(sym: Keysym) -> Option<egui::Key> {
    use egui::Key;
    Some(match sym {
        Keysym::Escape => Key::Escape,
        Keysym::Return | Keysym::KP_Enter => Key::Enter,
        Keysym::Tab | Keysym::ISO_Left_Tab => Key::Tab,
        Keysym::BackSpace => Key::Backspace,
        Keysym::Delete | Keysym::KP_Delete => Key::Delete,
        Keysym::Insert => Key::Insert,
        Keysym::Home | Keysym::KP_Home => Key::Home,
        Keysym::End | Keysym::KP_End => Key::End,
        Keysym::Page_Up | Keysym::KP_Page_Up => Key::PageUp,
        Keysym::Page_Down | Keysym::KP_Page_Down => Key::PageDown,
        Keysym::Left | Keysym::KP_Left => Key::ArrowLeft,
        Keysym::Right | Keysym::KP_Right => Key::ArrowRight,
        Keysym::Up | Keysym::KP_Up => Key::ArrowUp,
        Keysym::Down | Keysym::KP_Down => Key::ArrowDown,
        Keysym::space => Key::Space,
        Keysym::F1 => Key::F1,
        Keysym::F2 => Key::F2,
        Keysym::F3 => Key::F3,
        Keysym::F4 => Key::F4,
        Keysym::F5 => Key::F5,
        Keysym::F6 => Key::F6,
        Keysym::F7 => Key::F7,
        Keysym::F8 => Key::F8,
        Keysym::F9 => Key::F9,
        Keysym::F10 => Key::F10,
        Keysym::F11 => Key::F11,
        Keysym::F12 => Key::F12,
        // Latin letters and digits by name (shortcuts like Ctrl+A); other layouts type text only.
        other => {
            let c = other.key_char().filter(|c| c.is_ascii_alphanumeric())?;
            return Key::from_name(&c.to_ascii_uppercase().to_string());
        }
    })
}

impl CompositorHandler for Runner {
    fn scale_factor_changed(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_surface::WlSurface,
        _: i32,
    ) {
    }
    fn transform_changed(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_surface::WlSurface,
        _: wl_output::Transform,
    ) {
    }
    fn frame(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_surface::WlSurface, _: u32) {}
    fn surface_enter(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_surface::WlSurface,
        _: &wl_output::WlOutput,
    ) {
    }
    fn surface_leave(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_surface::WlSurface,
        _: &wl_output::WlOutput,
    ) {
    }
}

impl LayerShellHandler for Runner {
    /// The compositor closes layer surfaces it cannot place (no output yet, right after it
    /// started, or a monitor was removed): make the surface again, as explorer.exe re-creates
    /// its taskbar. Give up only if it keeps failing for a minute.
    fn closed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &LayerSurface) {
        if let Some(gl) = &mut self.gl {
            gl.surface = None;
        }
        self.closures += 1;
        if self.closures > 120 {
            self.exit = Some(Err("the compositor keeps closing the shell surface".into()));
            return;
        }
        self.recreate_at = Some(Instant::now() + Duration::from_millis(500));
    }

    fn configure(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &LayerSurface,
        configure: LayerSurfaceConfigure,
        _: u32,
    ) {
        let size = (configure.new_size.0.max(1), configure.new_size.1.max(1));
        if size != self.size {
            self.size = size;
            if let Some(gl) = &self.gl {
                if let Some(surface) = &gl.surface {
                    surface.resize(
                        &gl.context,
                        NonZeroU32::new(size.0).unwrap(),
                        NonZeroU32::new(size.1).unwrap(),
                    );
                }
            }
        }
        if self.gl.as_ref().is_some_and(|gl| gl.surface.is_none()) {
            if let Err(e) = self.attach() {
                self.exit = Some(Err(e));
                return;
            }
        }
        if self.gl.is_none() {
            match self.init_gl() {
                Ok(gl) => self.gl = Some(gl),
                Err(e) => {
                    self.exit = Some(Err(e));
                    return;
                }
            }
        }
        self.wake();
    }
}

impl SeatHandler for Runner {
    fn seat_state(&mut self) -> &mut SeatState {
        &mut self.seat_state
    }
    fn new_seat(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_seat::WlSeat) {}
    fn new_capability(
        &mut self,
        _: &Connection,
        qh: &QueueHandle<Self>,
        seat: wl_seat::WlSeat,
        capability: Capability,
    ) {
        if capability == Capability::Keyboard && self.keyboard.is_none() {
            self.keyboard = self.seat_state.get_keyboard(qh, &seat, None).ok();
        }
        if capability == Capability::Pointer && self.pointer.is_none() {
            let cursor_surface = self.compositor.create_surface(qh);
            self.pointer = self
                .seat_state
                .get_pointer_with_theme(
                    qh,
                    &seat,
                    self.shm.wl_shm(),
                    cursor_surface,
                    ThemeSpec::default(),
                )
                .ok();
        }
    }
    fn remove_capability(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: wl_seat::WlSeat,
        capability: Capability,
    ) {
        if capability == Capability::Keyboard {
            if let Some(k) = self.keyboard.take() {
                k.release();
            }
        }
        if capability == Capability::Pointer {
            self.pointer = None;
        }
    }
    fn remove_seat(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_seat::WlSeat) {}
}

impl KeyboardHandler for Runner {
    fn enter(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard,
        surface: &wl_surface::WlSurface,
        _: u32,
        _: &[u32],
        _: &[Keysym],
    ) {
        if surface == self.layer.wl_surface() {
            self.focused = true;
            self.events.push(egui::Event::WindowFocused(true));
            self.wake();
        }
    }
    fn leave(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard,
        surface: &wl_surface::WlSurface,
        _: u32,
    ) {
        if surface == self.layer.wl_surface() {
            self.focused = false;
            self.modifiers = egui::Modifiers::default();
            self.events.push(egui::Event::WindowFocused(false));
            self.wake();
        }
    }
    fn press_key(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard,
        _: u32,
        event: KeyEvent,
    ) {
        self.key(event, true, false);
    }
    fn repeat_key(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard,
        _: u32,
        event: KeyEvent,
    ) {
        self.key(event, true, true);
    }
    fn release_key(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard,
        _: u32,
        event: KeyEvent,
    ) {
        self.key(event, false, false);
    }
    fn update_modifiers(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard,
        _: u32,
        m: Modifiers,
        _: RawModifiers,
        _: u32,
    ) {
        self.modifiers = egui::Modifiers {
            alt: m.alt,
            ctrl: m.ctrl,
            shift: m.shift,
            mac_cmd: false,
            command: m.ctrl,
        };
    }
}

impl PointerHandler for Runner {
    fn pointer_frame(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_pointer::WlPointer,
        events: &[PointerEvent],
    ) {
        for event in events {
            if &event.surface != self.layer.wl_surface() {
                continue;
            }
            let pos = egui::pos2(event.position.0 as f32, event.position.1 as f32);
            match event.kind {
                PointerEventKind::Enter { .. } => {
                    self.apply_cursor();
                    self.pointer_pos = Some(pos);
                    self.events.push(egui::Event::PointerMoved(pos));
                }
                PointerEventKind::Leave { .. } => {
                    self.pointer_pos = None;
                    self.events.push(egui::Event::PointerGone);
                }
                PointerEventKind::Motion { .. } => {
                    self.pointer_pos = Some(pos);
                    self.events.push(egui::Event::PointerMoved(pos));
                }
                PointerEventKind::Press { button, .. }
                | PointerEventKind::Release { button, .. } => {
                    let pressed = matches!(event.kind, PointerEventKind::Press { .. });
                    let button = match button {
                        0x110 => egui::PointerButton::Primary,
                        0x111 => egui::PointerButton::Secondary,
                        0x112 => egui::PointerButton::Middle,
                        0x113 => egui::PointerButton::Extra1,
                        0x114 => egui::PointerButton::Extra2,
                        _ => continue,
                    };
                    self.events.push(egui::Event::PointerButton {
                        pos,
                        button,
                        pressed,
                        modifiers: self.modifiers,
                    });
                }
                PointerEventKind::Axis {
                    horizontal,
                    vertical,
                    ..
                } => {
                    self.events.push(egui::Event::MouseWheel {
                        unit: egui::MouseWheelUnit::Point,
                        delta: egui::vec2(-horizontal.absolute as f32, -vertical.absolute as f32),
                        modifiers: self.modifiers,
                    });
                }
            }
            self.wake();
        }
    }
}

impl OutputHandler for Runner {
    fn output_state(&mut self) -> &mut OutputState {
        &mut self.output_state
    }
    fn new_output(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}
    fn update_output(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}
    fn output_destroyed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}
}

impl ShmHandler for Runner {
    fn shm_state(&mut self) -> &mut Shm {
        &mut self.shm
    }
}

impl ProvidesRegistryState for Runner {
    fn registry(&mut self) -> &mut RegistryState {
        &mut self.registry
    }
    registry_handlers![OutputState, SeatState];
}

delegate_compositor!(Runner);
delegate_layer!(Runner);
delegate_seat!(Runner);
delegate_keyboard!(Runner);
delegate_pointer!(Runner);
delegate_shm!(Runner);
delegate_output!(Runner);
delegate_registry!(Runner);
