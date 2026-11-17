//! macOS: a menu bar item (`NSStatusItem`), as snapr's
//! (docs/MACOS-LEARNINGS.md 3.13). A left click is Avalonia's `Clicked`
//! (ShareX's tray left click action); a right or Control click drops down
//! the native menu ShareX builds from the tray menu (`BuildNativeMenu`).
//! Written without a Mac to run it on; see `docs/PORTING.md`.

use std::cell::{OnceCell, RefCell};

use objc2::rc::Retained;
use objc2::runtime::{AnyObject, NSObject};
use objc2::{AllocAnyThread, DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send, sel};
use objc2_app_kit::{
    NSApplication, NSControlStateValueOn, NSEventMask, NSEventModifierFlags, NSEventType, NSImage, NSMenu, NSMenuItem,
    NSStatusBar, NSStatusItem, NSVariableStatusItemLength,
};
use objc2_foundation::{NSData, NSSize, NSString};

use super::{Button, Callback, NativeEntry, TrayEvent};

struct Ivars {
    callback: Callback,
    status_item: OnceCell<Retained<NSStatusItem>>,
    menu: RefCell<Option<Retained<NSMenu>>>,
}

define_class!(
    // Receives clicks on the item and its menu's entries.
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "ShareXrsTrayTarget"]
    #[ivars = Ivars]
    struct Target;

    impl Target {
        #[unsafe(method(clicked:))]
        fn clicked(&self, _sender: Option<&AnyObject>) {
            let event = NSApplication::sharedApplication(self.mtm()).currentEvent();
            let right = event.is_some_and(|e| {
                e.r#type() == NSEventType::RightMouseUp || e.modifierFlags().contains(NSEventModifierFlags::Control)
            });
            if right {
                self.show_menu();
            } else {
                (self.ivars().callback)(TrayEvent::Up(Button::Left));
            }
        }

        #[unsafe(method(entryChosen:))]
        fn entry_chosen(&self, sender: Option<&NSMenuItem>) {
            if let Some(item) = sender {
                (self.ivars().callback)(TrayEvent::Command(item.tag() as usize));
            }
        }
    }
);

impl Target {
    fn new(mtm: MainThreadMarker, callback: Callback) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(Ivars { callback, status_item: OnceCell::new(), menu: RefCell::new(None) });
        unsafe { msg_send![super(this), init] }
    }

    /// Drops the menu down from the item, as if it were the item's own.
    fn show_menu(&self) {
        let Some(item) = self.ivars().status_item.get() else { return };
        let Some(menu) = self.ivars().menu.borrow().clone() else { return };
        item.setMenu(Some(&menu));
        if let Some(button) = item.button(self.mtm()) {
            // Returns once the menu closes.
            unsafe { button.performClick(None) };
        }
        item.setMenu(None);
    }
}

pub struct Tray {
    status_item: Retained<NSStatusItem>,
    target: Retained<Target>,
}

impl Tray {
    pub fn new(tooltip: &str, visible: bool, callback: Callback) -> Result<Self, String> {
        let mtm = MainThreadMarker::new().ok_or("the tray must be made on the main thread")?;
        let target = Target::new(mtm, callback);
        let status_item = NSStatusBar::systemStatusBar().statusItemWithLength(NSVariableStatusItemLength);
        if let Some(button) = status_item.button(mtm) {
            unsafe {
                button.setTarget(Some(&target));
                button.setAction(Some(sel!(clicked:)));
            }
            button.sendActionOn(NSEventMask::LeftMouseUp | NSEventMask::RightMouseUp);
        }
        let _ = target.ivars().status_item.set(status_item.clone());
        let mut tray = Self { status_item, target };
        tray.set_tooltip(tooltip);
        tray.set_icon(&super::app_icon(Self::icon_size()));
        tray.set_visible(visible);
        Ok(tray)
    }

    /// 18pt at 2x.
    pub fn icon_size() -> u32 {
        36
    }

    /// The picture at 18x18 points. ShareXrs's icon is in colour, so it's
    /// not a template image.
    pub fn set_icon(&mut self, image: &image::RgbaImage) {
        let Some(mtm) = MainThreadMarker::new() else { return };
        let mut png = Vec::new();
        if image.write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png).is_err() {
            return;
        }
        let Some(ns_image) = NSImage::initWithData(NSImage::alloc(), &NSData::with_bytes(&png)) else { return };
        ns_image.setSize(NSSize::new(18.0, 18.0));
        if let Some(button) = self.status_item.button(mtm) {
            button.setImage(Some(&ns_image));
        }
    }

    pub fn set_tooltip(&mut self, text: &str) {
        let Some(mtm) = MainThreadMarker::new() else { return };
        if let Some(button) = self.status_item.button(mtm) {
            button.setToolTip(Some(&NSString::from_str(text)));
        }
    }

    pub fn set_visible(&mut self, visible: bool) {
        self.status_item.setVisible(visible);
    }

    /// The native menu (`BuildNativeMenu`): headers, checks, submenus.
    pub fn set_menu(&mut self, entries: Vec<NativeEntry>) {
        let Some(mtm) = MainThreadMarker::new() else { return };
        let menu = build(mtm, &entries, &self.target);
        *self.target.ivars().menu.borrow_mut() = Some(menu);
    }
}

fn build(mtm: MainThreadMarker, entries: &[NativeEntry], target: &Target) -> Retained<NSMenu> {
    let menu = NSMenu::new(mtm);
    menu.setAutoenablesItems(false);
    for e in entries {
        if e.separator {
            menu.addItem(&NSMenuItem::separatorItem(mtm));
            continue;
        }
        let action = e.command.map(|_| sel!(entryChosen:));
        let item = unsafe {
            NSMenuItem::initWithTitle_action_keyEquivalent(
                NSMenuItem::alloc(mtm),
                &NSString::from_str(&e.header),
                action,
                &NSString::from_str(""),
            )
        };
        if let Some(i) = e.command {
            unsafe { item.setTarget(Some(target)) };
            item.setTag(i as isize);
        }
        item.setEnabled(e.enabled);
        if e.checked {
            item.setState(NSControlStateValueOn);
        }
        if let Some(children) = &e.children {
            item.setSubmenu(Some(&build(mtm, children, target)));
        }
        menu.addItem(&item);
    }
    menu
}

impl Drop for Tray {
    fn drop(&mut self) {
        NSStatusBar::systemStatusBar().removeStatusItem(&self.status_item);
    }
}

/// Docs/MACOS-LEARNINGS.md 3.5: a Dock icon and Cmd-Tab entry only while
/// the main window is open (`Regular`, brought to the front), none
/// otherwise (`Accessory`).
pub fn set_dock_icon(shown: bool) {
    use objc2_app_kit::NSApplicationActivationPolicy;
    let Some(mtm) = MainThreadMarker::new() else { return };
    let app = NSApplication::sharedApplication(mtm);
    if shown {
        app.setActivationPolicy(NSApplicationActivationPolicy::Regular);
        #[allow(deprecated)]
        app.activateIgnoringOtherApps(true);
    } else {
        app.setActivationPolicy(NSApplicationActivationPolicy::Accessory);
    }
}
