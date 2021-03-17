#[cfg(target_os = "macos")]
use cocoa::{
    appkit::{NSApp, NSApplication, NSMenu, NSMenuItem},
    base::{id, nil, YES},
    foundation::{NSAutoreleasePool, NSString},
};
use objc::{
    class,
    declare::ClassDecl,
    msg_send,
    runtime::{Object, Sel, BOOL},
    sel, sel_impl,
};
use std::{
    ffi::c_void,
    sync::{Arc, Mutex},
};

static APP_HANDLER_IVAR: &str = "GoDeskAppHandler";

lazy_static::lazy_static! {
    pub static ref SHOULD_OPEN_UNTITLED_FILE_CALLBACK: Arc<Mutex<Option<Box<dyn Fn() + Send>>>> = Default::default();
}

trait AppHandler {
    fn command(&mut self, cmd: u32);
}

struct DelegateState {
    handler: Option<Box<dyn AppHandler>>,
}

impl DelegateState {
    fn command(&mut self, command: u32) {
        if command == 0 {
            unsafe {
                let () = msg_send!(NSApp(), terminate: nil);
            }
        } else if let Some(inner) = self.handler.as_mut() {
            inner.command(command)
        }
    }
}

// https://github.com/xi-editor/druid/blob/master/druid-shell/src/platform/mac/application.rs
unsafe fn set_delegate(handler: Option<Box<dyn AppHandler>>) {
    let mut decl =
        ClassDecl::new("AppDelegate", class!(NSObject)).expect("App Delegate definition failed");
    decl.add_ivar::<*mut c_void>(APP_HANDLER_IVAR);

    decl.add_method(
        sel!(applicationDidFinishLaunching:),
        application_did_finish_launching as extern "C" fn(&mut Object, Sel, id),
    );

    decl.add_method(
        sel!(applicationShouldOpenUntitledFile:),
        application_should_handle_open_untitled_file as extern "C" fn(&mut Object, Sel, id) -> BOOL,
    );

    decl.add_method(
        sel!(handleMenuItem:),
        handle_menu_item as extern "C" fn(&mut Object, Sel, id),
    );
    let decl = decl.register();
    let delegate: id = msg_send![decl, alloc];
    let () = msg_send![delegate, init];
    let state = DelegateState { handler };
    let handler_ptr = Box::into_raw(Box::new(state));
    (*delegate).set_ivar(APP_HANDLER_IVAR, handler_ptr as *mut c_void);
    let () = msg_send![NSApp(), setDelegate: delegate];
}

extern "C" fn application_did_finish_launching(_this: &mut Object, _: Sel, _notification: id) {
    unsafe {
        let () = msg_send![NSApp(), activateIgnoringOtherApps: YES];
    }
}

extern "C" fn application_should_handle_open_untitled_file(
    _this: &mut Object,
    _: Sel,
    _sender: id,
) -> BOOL {
    if let Some(callback) = SHOULD_OPEN_UNTITLED_FILE_CALLBACK.lock().unwrap().as_ref() {
        callback();
    }
    YES
}

/// This handles menu items in the case that all windows are closed.
extern "C" fn handle_menu_item(this: &mut Object, _: Sel, item: id) {
    unsafe {
        let tag: isize = msg_send![item, tag];
        if tag == 0 {
            let inner: *mut c_void = *this.get_ivar(APP_HANDLER_IVAR);
            let inner = &mut *(inner as *mut DelegateState);
            (*inner).command(tag as u32);
        } else if tag == 1 {
            crate::run_me(Vec::<String>::new()).ok();
        }
    }
}

pub fn make_menubar() {
    unsafe {
        let _pool = NSAutoreleasePool::new(nil);
        set_delegate(None);
        let menubar = NSMenu::new(nil).autorelease();
        let app_menu_item = NSMenuItem::new(nil).autorelease();
        menubar.addItem_(app_menu_item);
        let app_menu = NSMenu::new(nil).autorelease();
        let quit_title =
            NSString::alloc(nil).init_str(&format!("Quit {}", hbb_common::config::APP_NAME));
        let quit_action = sel!(handleMenuItem:);
        let quit_key = NSString::alloc(nil).init_str("q");
        let quit_item = NSMenuItem::alloc(nil)
            .initWithTitle_action_keyEquivalent_(quit_title, quit_action, quit_key)
            .autorelease();
        let () = msg_send![quit_item, setTag: 0];
        /*
        if !enabled {
            let () = msg_send![quit_item, setEnabled: NO];
        }

        if selected {
            let () = msg_send![quit_item, setState: 1_isize];
        }
        let () = msg_send![item, setTag: id as isize];
        */
        app_menu.addItem_(quit_item);
        if std::env::args().len() > 1 {
            let new_title = NSString::alloc(nil).init_str("New Window");
            let new_action = sel!(handleMenuItem:);
            let new_key = NSString::alloc(nil).init_str("n");
            let new_item = NSMenuItem::alloc(nil)
                .initWithTitle_action_keyEquivalent_(new_title, new_action, new_key)
                .autorelease();
            let () = msg_send![new_item, setTag: 1];
            app_menu.addItem_(new_item);
        }
        app_menu_item.setSubmenu_(app_menu);
        NSApp().setMainMenu_(menubar);
    }
}
