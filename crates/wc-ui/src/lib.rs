//! WinCraft design system, shared by the shell and the system applications.
//!
//! Visual language: between Windows 10/11 and ReactOS. Clear structure and labelled
//! taskbar buttons (Win10/ReactOS); Win11 surfaces, 4–8 px corners and an accent bar on
//! selected items. All artwork is drawn from vectors here; there are no Microsoft assets.
pub mod icons;
pub mod theme;
pub mod wallpaper;
pub mod widgets;

pub use icons::{paint_icon, Icon};
pub use theme::{Palette, Theme};
