use crate::{Error, SystrayEvent};
use glib;
use gtk::{
    self, MenuShellExt, GtkMenuItemExt, WidgetExt
};
use libappindicator::{AppIndicator, AppIndicatorStatus};
use std::{
    self,
    cell::RefCell,
    collections::HashMap,
    sync::mpsc::{channel, Sender},
    thread,
};

// Gtk specific struct that will live only in the Gtk thread, since a lot of the
// base types involved don't implement Send (for good reason).
pub struct GtkSystrayApp {
    menu: gtk::Menu,
    ai: RefCell<AppIndicator>,
    menu_items: RefCell<HashMap<u32, gtk::MenuItem>>,
    event_tx: Sender<SystrayEvent>,
}

thread_local!(static GTK_STASH: RefCell<Option<GtkSystrayApp>> = RefCell::new(None));

pub struct MenuItemInfo {
    mid: u32,
    title: String,
    tooltip: String,
    disabled: bool,
    checked: bool,
}

type Callback = Box<(Fn(&GtkSystrayApp) -> () + 'static)>;

// Convenience function to clean up thread local unwrapping
fn run_on_gtk_thread<F>(f: F)
where
    F: std::ops::Fn(&GtkSystrayApp) -> () + Send + 'static,
{
    // Note this is glib, not gtk. Calling gtk::idle_add will panic us due to
    // being on different threads. glib::idle_add can run across threads.
    glib::idle_add(move || {
        GTK_STASH.with(|stash| {
            let stash = stash.borrow();
            let stash = stash.as_ref();
            if let Some(stash) = stash {
                f(stash);
            }
        });
        gtk::prelude::Continue(false)
    });
}

impl GtkSystrayApp {
    pub fn new(event_tx: Sender<SystrayEvent>) -> Result<GtkSystrayApp, Error> {
        if let Err(e) = gtk::init() {
            return Err(Error::OsError(format!("{}", "Gtk init error!")));
        }
        let mut m = gtk::Menu::new();
        let mut ai = AppIndicator::new("", "");
        ai.set_status(AppIndicatorStatus::Active);
        ai.set_menu(&mut m);
        Ok(GtkSystrayApp {
            menu: m,
            ai: RefCell::new(ai),
            menu_items: RefCell::new(HashMap::new()),
            event_tx: event_tx,
        })
    }

    pub fn systray_menu_selected(&self, menu_id: u32) {
        self.event_tx
            .send(SystrayEvent {
                menu_index: menu_id as u32,
            })
            .ok();
    }

    pub fn add_menu_separator(&self, item_idx: u32) {
        //let mut menu_items = self.menu_items.borrow_mut();
        let m = gtk::SeparatorMenuItem::new();
        self.menu.append(&m);
        //menu_items.insert(item_idx, m);
        self.menu.show_all();
    }

    pub fn add_menu_entry(&self, item_idx: u32, item_name: &str) {
        let mut menu_items = self.menu_items.borrow_mut();
        if menu_items.contains_key(&item_idx) {
            let m: &gtk::MenuItem = menu_items.get(&item_idx).unwrap();
            m.set_label(item_name);
            self.menu.show_all();
            return;
        }
        let m = gtk::MenuItem::new_with_label(item_name);
        self.menu.append(&m);
        m.connect_activate(move |_| {
            run_on_gtk_thread(move |stash: &GtkSystrayApp| {
                stash.systray_menu_selected(item_idx);
            });
        });
        menu_items.insert(item_idx, m);
        self.menu.show_all();
    }

    pub fn set_icon_from_file(&self, file: &str) {
        let mut ai = self.ai.borrow_mut();
        ai.set_icon_full(file, "icon");
    }
}

pub struct Window {
    gtk_loop: Option<thread::JoinHandle<()>>,
}

impl Window {
    pub fn new(event_tx: Sender<SystrayEvent>) -> Result<Window, Error> {
        let (tx, rx) = channel();
        let gtk_loop = thread::spawn(move || {
            GTK_STASH.with(|stash| match GtkSystrayApp::new(event_tx) {
                Ok(data) => {
                    (*stash.borrow_mut()) = Some(data);
                    tx.send(Ok(()));
                }
                Err(e) => {
                    tx.send(Err(e));
                    return;
                }
            });
            gtk::main();
        });
        match rx.recv().unwrap() {
            Ok(()) => Ok(Window {
                gtk_loop: Some(gtk_loop),
            }),
            Err(e) => Err(e),
        }
    }

    pub fn add_menu_entry(&self, item_idx: u32, item_name: &str) -> Result<(), Error> {
        let n = item_name.to_owned().clone();
        run_on_gtk_thread(move |stash: &GtkSystrayApp| {
            stash.add_menu_entry(item_idx, &n);
        });
        Ok(())
    }

    pub fn add_menu_separator(&self, item_idx: u32) -> Result<(), Error> {
        run_on_gtk_thread(move |stash: &GtkSystrayApp| {
            stash.add_menu_separator(item_idx);
        });
        Ok(())
    }

    pub fn set_icon_from_file(&self, file: &str) -> Result<(), Error> {
        let n = file.to_owned().clone();
        run_on_gtk_thread(move |stash: &GtkSystrayApp| {
            stash.set_icon_from_file(&n);
        });
        Ok(())
    }

    pub fn set_icon_from_resource(&self, resource: &str) -> Result<(), Error> {
        panic!("Not implemented on this platform!");
    }

    pub fn shutdown(&self) -> Result<(), Error> {
        Ok(())
    }

    pub fn set_tooltip(&self, tooltip: &str) -> Result<(), Error> {
        panic!("Not implemented on this platform!");
    }

    pub fn quit(&self) {
        glib::idle_add(|| {
            gtk::main_quit();
            glib::Continue(false)
        });
    }
}
