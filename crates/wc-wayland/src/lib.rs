//! Wayland client side of the WinCraft shell (ADR-018).
//!
//! - [`layer`]: egui on wlr-layer-shell surfaces, for the desktop and the taskbar;
//! - [`toplevels`]: the window list the taskbar shows (wlr-foreign-toplevel-management).
//!
//! The compositor (labwc) draws window frames and places ordinary windows; everything
//! here is an unprivileged client of it.
pub mod layer;
pub mod toplevels;

pub use toplevels::{WindowInfo, WindowList};
