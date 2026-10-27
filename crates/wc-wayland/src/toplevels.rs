//! The taskbar's view of top-level windows (wlr-foreign-toplevel-management).
//!
//! The compositor reports every application window, native Wayland and XWayland (Windows
//! programs) alike; shell surfaces (desktop, taskbar) are layer surfaces and never appear.
use smithay_client_toolkit::reexports::{
    client::{
        event_created_child,
        globals::{registry_queue_init, GlobalListContents},
        protocol::{wl_registry, wl_seat::WlSeat},
        Connection, Dispatch, EventQueue, Proxy, QueueHandle,
    },
    protocols_wlr::foreign_toplevel::v1::client::{
        zwlr_foreign_toplevel_handle_v1::{self, ZwlrForeignToplevelHandleV1},
        zwlr_foreign_toplevel_manager_v1::{self, ZwlrForeignToplevelManagerV1},
    },
};

pub type Error = Box<dyn std::error::Error + Send + Sync>;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WindowInfo {
    /// Stable for the lifetime of the window within this connection.
    pub id: u32,
    pub title: String,
    /// Wayland app_id; for XWayland windows the WM_CLASS (e.g. "win32-demo.exe").
    pub app_id: String,
    pub minimized: bool,
    pub active: bool,
}

struct Entry {
    handle: ZwlrForeignToplevelHandleV1,
    info: WindowInfo,
    /// Owned windows (dialogs with a parent) do not get their own taskbar button.
    has_parent: bool,
    /// The first `done` event has arrived: title and state are complete.
    ready: bool,
}

#[derive(Default)]
struct State {
    entries: Vec<Entry>,
    next_id: u32,
    finished: bool,
}

impl State {
    fn entry(&mut self, handle: &ZwlrForeignToplevelHandleV1) -> Option<&mut Entry> {
        self.entries.iter_mut().find(|e| &e.handle == handle)
    }
}

pub struct WindowList {
    queue: EventQueue<State>,
    state: State,
    manager: ZwlrForeignToplevelManagerV1,
    seat: Option<WlSeat>,
    /// Windows minimized by "Show desktop", restored by the next one.
    shown_desktop: Vec<u32>,
}

impl WindowList {
    pub fn connect() -> Result<Self, Error> {
        let conn = Connection::connect_to_env()?;
        let (globals, mut queue) = registry_queue_init::<State>(&conn)?;
        let qh = queue.handle();
        let manager = globals
            .bind::<ZwlrForeignToplevelManagerV1, _, _>(&qh, 1..=3, ())
            .map_err(|e| format!("the compositor offers no window list: {e}"))?;
        let seat = globals.bind::<WlSeat, _, _>(&qh, 1..=7, ()).ok();
        let mut state = State::default();
        queue.roundtrip(&mut state)?;
        Ok(Self {
            queue,
            state,
            manager,
            seat,
            shown_desktop: vec![],
        })
    }

    /// Applies everything the compositor reported since the last call.
    pub fn refresh(&mut self) -> Result<(), Error> {
        self.queue.roundtrip(&mut self.state)?;
        if self.state.finished {
            return Err("the compositor ended the window list".into());
        }
        Ok(())
    }

    /// Windows that belong on the taskbar, in creation order.
    pub fn windows(&self) -> Vec<WindowInfo> {
        self.state
            .entries
            .iter()
            .filter(|e| e.ready && !e.has_parent)
            .map(|e| e.info.clone())
            .collect()
    }

    pub fn active(&self) -> Option<u32> {
        self.state
            .entries
            .iter()
            .find(|e| e.info.active)
            .map(|e| e.info.id)
    }

    fn handle(&self, id: u32) -> Option<&ZwlrForeignToplevelHandleV1> {
        self.state
            .entries
            .iter()
            .find(|e| e.info.id == id)
            .map(|e| &e.handle)
    }

    fn flush(&self) -> Result<(), Error> {
        self.manager
            .backend()
            .upgrade()
            .ok_or("connection lost")?
            .flush()?;
        Ok(())
    }

    /// Brings a window to the front, restoring it if minimized.
    pub fn activate(&mut self, id: u32) -> Result<(), Error> {
        let (Some(handle), Some(seat)) = (self.handle(id), &self.seat) else {
            return Ok(());
        };
        handle.unset_minimized();
        handle.activate(seat);
        self.shown_desktop.clear();
        self.flush()
    }

    pub fn minimize(&self, id: u32) -> Result<(), Error> {
        if let Some(handle) = self.handle(id) {
            handle.set_minimized();
        }
        self.flush()
    }

    pub fn close(&self, id: u32) -> Result<(), Error> {
        if let Some(handle) = self.handle(id) {
            handle.close();
        }
        self.flush()
    }

    /// Minimizes every window; a second call restores the ones it minimized (Win+D).
    pub fn toggle_desktop(&mut self) -> Result<(), Error> {
        if self.shown_desktop.is_empty() {
            for entry in self
                .state
                .entries
                .iter()
                .filter(|e| e.ready && !e.info.minimized)
            {
                entry.handle.set_minimized();
                self.shown_desktop.push(entry.info.id);
            }
        } else {
            for id in std::mem::take(&mut self.shown_desktop) {
                if let Some(handle) = self.handle(id) {
                    handle.unset_minimized();
                }
            }
        }
        self.flush()
    }
}

impl Drop for WindowList {
    fn drop(&mut self) {
        self.manager.stop();
        let _ = self.flush();
    }
}

impl Dispatch<wl_registry::WlRegistry, GlobalListContents> for State {
    fn event(
        _: &mut Self,
        _: &wl_registry::WlRegistry,
        _: wl_registry::Event,
        _: &GlobalListContents,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<WlSeat, ()> for State {
    fn event(
        _: &mut Self,
        _: &WlSeat,
        _: <WlSeat as Proxy>::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<ZwlrForeignToplevelManagerV1, ()> for State {
    fn event(
        state: &mut Self,
        _: &ZwlrForeignToplevelManagerV1,
        event: zwlr_foreign_toplevel_manager_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            zwlr_foreign_toplevel_manager_v1::Event::Toplevel { toplevel } => {
                state.next_id += 1;
                state.entries.push(Entry {
                    handle: toplevel,
                    info: WindowInfo {
                        id: state.next_id,
                        title: String::new(),
                        app_id: String::new(),
                        minimized: false,
                        active: false,
                    },
                    has_parent: false,
                    ready: false,
                });
            }
            zwlr_foreign_toplevel_manager_v1::Event::Finished => state.finished = true,
            _ => {}
        }
    }

    event_created_child!(State, ZwlrForeignToplevelManagerV1, [
        zwlr_foreign_toplevel_manager_v1::EVT_TOPLEVEL_OPCODE => (ZwlrForeignToplevelHandleV1, ())
    ]);
}

impl Dispatch<ZwlrForeignToplevelHandleV1, ()> for State {
    fn event(
        state: &mut Self,
        handle: &ZwlrForeignToplevelHandleV1,
        event: zwlr_foreign_toplevel_handle_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        use zwlr_foreign_toplevel_handle_v1::Event;
        if let Event::Closed = event {
            state.entries.retain(|e| &e.handle != handle);
            handle.destroy();
            return;
        }
        let Some(entry) = state.entry(handle) else {
            return;
        };
        match event {
            Event::Title { title } => entry.info.title = title,
            Event::AppId { app_id } => entry.info.app_id = app_id,
            Event::State { state: raw } => {
                let states: Vec<u32> = raw
                    .chunks_exact(4)
                    .map(|c| u32::from_ne_bytes([c[0], c[1], c[2], c[3]]))
                    .collect();
                let has = |s: zwlr_foreign_toplevel_handle_v1::State| states.contains(&(s as u32));
                entry.info.minimized = has(zwlr_foreign_toplevel_handle_v1::State::Minimized);
                entry.info.active = has(zwlr_foreign_toplevel_handle_v1::State::Activated);
            }
            Event::Parent { parent } => entry.has_parent = parent.is_some(),
            Event::Done => entry.ready = true,
            _ => {}
        }
    }
}
