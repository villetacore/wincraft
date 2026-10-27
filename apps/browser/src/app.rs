//! The browser window: tabs (one WebKit view each), address bar, downloads.
use crate::address;
use std::{
    cell::{Cell, RefCell},
    path::PathBuf,
    process::Command,
    rc::{Rc, Weak},
};
use webkit6::{gio, glib, gtk, prelude::*, Download, LoadEvent, NetworkSession, WebView};

const APP_ID: &str = "org.wincraft.Browser";
const NAME: &str = "WinCraft Browser";

struct Browser {
    window: gtk::ApplicationWindow,
    notebook: gtk::Notebook,
    address: gtk::Entry,
    /// While the user edits the address, page navigation does not overwrite it.
    editing: Cell<bool>,
    back: gtk::Button,
    forward: gtk::Button,
    reload: gtk::Button,
    progress: gtk::ProgressBar,
    downloads: gtk::Revealer,
    download_label: gtk::Label,
    download_open: gtk::Button,
    download_folder: gtk::Button,
    last_download: RefCell<Option<PathBuf>>,
    session: NetworkSession,
}

fn home_dir() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/tmp"))
}

fn flat_button(label: &str, tooltip: &str) -> gtk::Button {
    let button = gtk::Button::with_label(label);
    button.add_css_class("flat");
    button.set_tooltip_text(Some(tooltip));
    button
}

impl Browser {
    fn current(&self) -> Option<WebView> {
        self.notebook
            .nth_page(self.notebook.current_page())
            .and_then(|w| w.downcast::<WebView>().ok())
    }

    /// Address bar, buttons and window title follow the selected tab.
    fn sync(&self) {
        let Some(view) = self.current() else { return };
        if !self.editing.get() {
            self.address
                .set_text(view.uri().as_deref().unwrap_or_default());
        }
        self.back.set_sensitive(view.can_go_back());
        self.forward.set_sensitive(view.can_go_forward());
        self.reload
            .set_label(if view.is_loading() { "✕" } else { "⟳" });
        self.reload.set_tooltip_text(Some(if view.is_loading() {
            "Stop (Esc)"
        } else {
            "Refresh (F5)"
        }));
        let loading = view.is_loading() && view.estimated_load_progress() < 1.0;
        self.progress.set_visible(loading);
        self.progress.set_fraction(view.estimated_load_progress());
        let title = view
            .title()
            .filter(|t| !t.is_empty())
            .map(|t| t.to_string());
        self.window.set_title(Some(&match title {
            Some(t) => format!("{t} — {NAME}"),
            None => NAME.into(),
        }));
    }

    fn open_tab(self: &Rc<Self>, uri: &str, related: Option<&WebView>) -> WebView {
        let builder = WebView::builder();
        let view = match related {
            // Pages opened by a page (target=_blank) share its process and session.
            Some(parent) => builder.related_view(parent).build(),
            None => builder.network_session(&self.session).build(),
        };
        view.set_hexpand(true);
        view.set_vexpand(true);
        if let Some(settings) = WebViewExt::settings(&view) {
            settings.set_enable_developer_extras(false);
            settings.set_enable_smooth_scrolling(true);
        }

        let title = gtk::Label::builder()
            .label("New tab")
            .ellipsize(gtk::pango::EllipsizeMode::End)
            .width_chars(14)
            .max_width_chars(22)
            .build();
        let close = flat_button("✕", "Close tab (Ctrl+W)");
        let tab = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        tab.append(&title);
        tab.append(&close);
        let page = self.notebook.append_page(&view, Some(&tab));
        self.notebook.set_tab_reorderable(&view, true);

        let me = Rc::downgrade(self);
        let closing = view.clone();
        close.connect_clicked(move |_| with(&me, |b| b.close_tab(&closing)));
        let me = Rc::downgrade(self);
        view.connect_title_notify(move |v| {
            let text = v
                .title()
                .filter(|t| !t.is_empty())
                .map(|t| t.to_string())
                .unwrap_or_else(|| "New tab".into());
            title.set_label(&text);
            with(&me, |b| b.sync());
        });
        for notify in ["uri", "estimated-load-progress", "is-loading"] {
            let me = Rc::downgrade(self);
            view.connect_notify_local(Some(notify), move |_, _| with(&me, |b| b.sync()));
        }
        let me = Rc::downgrade(self);
        view.connect_load_changed(move |_, event| {
            if event == LoadEvent::Finished || event == LoadEvent::Committed {
                with(&me, |b| b.sync());
            }
        });
        let me = Rc::downgrade(self);
        view.connect_create(move |parent, _| {
            let me = me.upgrade()?;
            let child = me.open_tab("about:blank", Some(parent));
            Some(child.upcast())
        });
        let me = Rc::downgrade(self);
        view.connect_close(move |v| with(&me, |b| b.close_tab(v)));

        if uri != "about:blank" {
            view.load_uri(uri);
        }
        self.notebook.set_current_page(Some(page));
        view
    }

    fn close_tab(&self, view: &WebView) {
        if let Some(page) = self.notebook.page_num(view) {
            self.notebook.remove_page(Some(page));
        }
        if self.notebook.n_pages() == 0 {
            self.window.close();
        }
    }

    fn navigate(&self, input: &str) {
        if let Some(view) = self.current() {
            view.load_uri(&address::resolve(input));
            view.grab_focus();
        }
    }

    /// Downloads go to the user's Downloads folder (C:\users\user\Downloads for Windows
    /// programs too); the bar below the page offers to open the file, which runs installers.
    fn track_download(self: &Rc<Self>, download: &Download) {
        let failed = Rc::new(Cell::new(false));
        let me = Rc::downgrade(self);
        download.connect_decide_destination(move |d, suggested| {
            let dir = home_dir().join("Downloads");
            let _ = std::fs::create_dir_all(&dir);
            let path =
                address::unique_path(&dir, &address::safe_file_name(suggested), |p| p.exists());
            d.set_destination(&path.to_string_lossy());
            with(&me, |b| {
                b.download_label.set_label(&format!(
                    "Downloading {}…",
                    path.file_name().unwrap_or_default().to_string_lossy()
                ));
                b.download_open.set_visible(false);
                b.download_folder.set_visible(false);
                b.downloads.set_reveal_child(true);
                *b.last_download.borrow_mut() = Some(path.clone());
            });
            true
        });
        let me = Rc::downgrade(self);
        let failure = failed.clone();
        download.connect_failed(move |_, error| {
            failure.set(true);
            with(&me, |b| {
                b.download_label
                    .set_label(&format!("Download failed: {error}"))
            });
        });
        let me = Rc::downgrade(self);
        download.connect_finished(move |_| {
            if failed.get() {
                return;
            }
            with(&me, |b| {
                let name = b
                    .last_download
                    .borrow()
                    .as_ref()
                    .and_then(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
                    .unwrap_or_default();
                b.download_label
                    .set_label(&format!("{name} — download complete"));
                b.download_open.set_visible(true);
                b.download_folder.set_visible(true);
            });
        });
    }
}

fn with(browser: &Weak<Browser>, f: impl FnOnce(&Rc<Browser>)) {
    if let Some(b) = browser.upgrade() {
        f(&b);
    }
}

fn build(app: &gtk::Application, start: &[String]) {
    let data = home_dir().join(".local/share/wincraft/browser");
    let cache = home_dir().join(".cache/wincraft/browser");
    let session = NetworkSession::new(data.to_str(), cache.to_str());

    let window = gtk::ApplicationWindow::builder()
        .application(app)
        .title(NAME)
        .default_width(1180)
        .default_height(740)
        .build();
    let back = flat_button("←", "Back (Alt+Left)");
    let forward = flat_button("→", "Forward (Alt+Right)");
    let reload = flat_button("⟳", "Refresh (F5)");
    let home = flat_button("⌂", "Home");
    let new_tab = flat_button("+", "New tab (Ctrl+T)");
    let address = gtk::Entry::builder()
        .hexpand(true)
        .placeholder_text("Search or enter web address")
        .build();
    let header = gtk::HeaderBar::new();
    for b in [&back, &forward, &reload, &home] {
        header.pack_start(b);
    }
    header.set_title_widget(Some(&address));
    header.pack_end(&new_tab);
    window.set_titlebar(Some(&header));

    let notebook = gtk::Notebook::builder()
        .scrollable(true)
        .show_border(false)
        .vexpand(true)
        .build();
    let progress = gtk::ProgressBar::new();
    progress.set_visible(false);
    let download_label = gtk::Label::builder().hexpand(true).xalign(0.0).build();
    let download_open = gtk::Button::with_label("Open");
    download_open.add_css_class("suggested-action");
    let download_folder = gtk::Button::with_label("Show in folder");
    let download_close = flat_button("✕", "Close");
    let bar = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(8)
        .margin_start(12)
        .margin_end(6)
        .margin_top(6)
        .margin_bottom(6)
        .build();
    for w in [
        download_label.upcast_ref::<gtk::Widget>(),
        download_open.upcast_ref(),
        download_folder.upcast_ref(),
        download_close.upcast_ref(),
    ] {
        bar.append(w);
    }
    let downloads = gtk::Revealer::builder()
        .child(&bar)
        .transition_type(gtk::RevealerTransitionType::SlideUp)
        .build();
    let content = gtk::Box::new(gtk::Orientation::Vertical, 0);
    content.append(&progress);
    content.append(&notebook);
    content.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
    content.append(&downloads);
    window.set_child(Some(&content));

    let browser = Rc::new(Browser {
        window: window.clone(),
        notebook: notebook.clone(),
        address: address.clone(),
        editing: Cell::new(false),
        back: back.clone(),
        forward: forward.clone(),
        reload: reload.clone(),
        progress,
        downloads: downloads.clone(),
        download_label,
        download_open: download_open.clone(),
        download_folder: download_folder.clone(),
        last_download: RefCell::new(None),
        session: session.clone(),
    });

    let me = Rc::downgrade(&browser);
    session.connect_download_started(move |_, download| with(&me, |b| b.track_download(download)));
    let me = Rc::downgrade(&browser);
    download_open.connect_clicked(move |_| {
        with(&me, |b| {
            if let Some(path) = b.last_download.borrow().clone() {
                // ShellExecute: setup.exe installs through Wine, documents open in their app.
                let _ = Command::new("start").arg("--").arg(path).spawn();
            }
        })
    });
    let me = Rc::downgrade(&browser);
    download_folder.connect_clicked(move |_| {
        with(&me, |b| {
            if let Some(dir) = b
                .last_download
                .borrow()
                .as_ref()
                .and_then(|p| p.parent().map(PathBuf::from))
            {
                let _ = Command::new("explorer").arg("--folder").arg(dir).spawn();
            }
        })
    });
    let reveal = downloads.clone();
    download_close.connect_clicked(move |_| reveal.set_reveal_child(false));

    let me = Rc::downgrade(&browser);
    address.connect_activate(move |entry| {
        with(&me, |b| {
            b.editing.set(false);
            b.navigate(&entry.text());
        })
    });
    let focus = gtk::EventControllerFocus::new();
    let me = Rc::downgrade(&browser);
    focus.connect_enter(move |_| with(&me, |b| b.editing.set(true)));
    let me = Rc::downgrade(&browser);
    focus.connect_leave(move |_| {
        with(&me, |b| {
            b.editing.set(false);
            b.sync();
        })
    });
    address.add_controller(focus);

    let me = Rc::downgrade(&browser);
    back.connect_clicked(move |_| {
        with(&me, |b| {
            if let Some(v) = b.current() {
                v.go_back()
            }
        })
    });
    let me = Rc::downgrade(&browser);
    forward.connect_clicked(move |_| {
        with(&me, |b| {
            if let Some(v) = b.current() {
                v.go_forward()
            }
        })
    });
    let me = Rc::downgrade(&browser);
    reload.connect_clicked(move |_| {
        with(&me, |b| {
            if let Some(v) = b.current() {
                if v.is_loading() {
                    v.stop_loading()
                } else {
                    v.reload()
                }
            }
        })
    });
    let me = Rc::downgrade(&browser);
    home.connect_clicked(move |_| with(&me, |b| b.navigate(address::HOME)));
    let me = Rc::downgrade(&browser);
    new_tab.connect_clicked(move |_| {
        with(&me, |b| {
            b.open_tab(address::HOME, None);
        })
    });
    let me = Rc::downgrade(&browser);
    notebook.connect_switch_page(move |_, _, _| {
        let me = me.clone();
        // The page number changes after the signal; sync once it has.
        glib::idle_add_local_once(move || with(&me, |b| b.sync()));
    });

    // Keyboard shortcuts, as in Windows browsers.
    let actions: [(&str, &[&str], Box<dyn Fn(&Rc<Browser>)>); 8] = [
        (
            "new-tab",
            &["<Ctrl>t"],
            Box::new(|b| {
                b.open_tab(address::HOME, None);
            }),
        ),
        (
            "close-tab",
            &["<Ctrl>w", "<Ctrl>F4"],
            Box::new(|b| {
                if let Some(v) = b.current() {
                    b.close_tab(&v)
                }
            }),
        ),
        (
            "address",
            &["<Ctrl>l", "F6", "<Alt>d"],
            Box::new(|b| {
                b.address.grab_focus();
            }),
        ),
        (
            "reload",
            &["F5", "<Ctrl>r"],
            Box::new(|b| {
                if let Some(v) = b.current() {
                    v.reload()
                }
            }),
        ),
        (
            "stop",
            &["Escape"],
            Box::new(|b| {
                if let Some(v) = b.current() {
                    v.stop_loading()
                }
            }),
        ),
        (
            "back",
            &["<Alt>Left"],
            Box::new(|b| {
                if let Some(v) = b.current() {
                    v.go_back()
                }
            }),
        ),
        (
            "forward",
            &["<Alt>Right"],
            Box::new(|b| {
                if let Some(v) = b.current() {
                    v.go_forward()
                }
            }),
        ),
        (
            "next-tab",
            &["<Ctrl>Tab"],
            Box::new(|b| {
                let n = b.notebook.n_pages();
                if n > 0 {
                    let next = b.notebook.current_page().map_or(0, |p| (p + 1) % n);
                    b.notebook.set_current_page(Some(next));
                }
            }),
        ),
    ];
    for (name, accels, run) in actions {
        let action = gio::SimpleAction::new(name, None);
        let me = Rc::downgrade(&browser);
        action.connect_activate(move |_, _| with(&me, |b| run(b)));
        window.add_action(&action);
        app.set_accels_for_action(&format!("win.{name}"), accels);
    }

    if start.is_empty() {
        browser.open_tab(address::HOME, None);
    }
    for target in start {
        browser.open_tab(&address::resolve(target), None);
    }
    window.present();
}

pub fn run() -> glib::ExitCode {
    // Software rendering in the VM: without a GPU, WebKit's DMA-BUF renderer has nothing to
    // share buffers with; shared memory works everywhere (removed with GPU-01).
    if std::env::var_os("WEBKIT_DISABLE_DMABUF_RENDERER").is_none() {
        // SAFETY: called before GTK or WebKit start any thread.
        unsafe { std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1") };
    }
    let start: Vec<String> = std::env::args().skip(1).collect();
    // One window per process, like the rest of the shell (no D-Bus single instance).
    let app = gtk::Application::builder()
        .application_id(APP_ID)
        .flags(gio::ApplicationFlags::NON_UNIQUE)
        .build();
    app.connect_activate(move |app| build(app, &start));
    app.run_with_args::<&str>(&[])
}
