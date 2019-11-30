use crate::input::*;
use crate::os::wayland::connection::WaylandConnection;
use failure::Fallible;
use smithay_client_toolkit as toolkit;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use toolkit::keyboard::{
    map_keyboard_auto_with_repeat, Event as KbEvent, KeyRepeatEvent, KeyRepeatKind, KeyState,
    ModifiersState,
};
use wayland_client::protocol::wl_seat::WlSeat;
use wayland_client::protocol::wl_surface::WlSurface;

#[derive(Default)]
struct Inner {
    active_surface_id: u32,
    surface_to_window_id: HashMap<u32, usize>,
}

impl Inner {
    fn handle_event(&mut self, evt: KbEvent) {
        if let KbEvent::Enter { surface, .. } = &evt {
            self.active_surface_id = surface.as_ref().id();
        }

        if let Some(event) = KeyboardEvent::from_event(evt) {
            self.dispatch_to_window(event);
        }
    }

    fn handle_repeat(&mut self, rawkey: u32, keysym: u32, utf8: Option<String>) {
        self.dispatch_to_window(KeyboardEvent::Key {
            serial: 0,
            rawkey,
            keysym,
            is_down: true,
            utf8,
        });
    }

    fn dispatch_to_window(&mut self, evt: KeyboardEvent) {
        if let Some(window_id) = self.surface_to_window_id.get(&self.active_surface_id) {
            let mut evt = Some(evt);
            WaylandConnection::with_window_inner(*window_id, move |inner| {
                inner.handle_keyboard_event(evt.take().unwrap());
                Ok(())
            });
        }
    }
}

#[derive(Clone)]
pub struct KeyboardDispatcher {
    inner: Arc<Mutex<Inner>>,
}

impl KeyboardDispatcher {
    pub fn register(seat: &WlSeat) -> Fallible<Self> {
        let inner = Arc::new(Mutex::new(Inner::default()));

        map_keyboard_auto_with_repeat(
            &seat,
            KeyRepeatKind::System,
            {
                let inner = Arc::clone(&inner);
                move |evt: KbEvent, _| {
                    inner.lock().unwrap().handle_event(evt);
                }
            },
            {
                let inner = Arc::clone(&inner);
                move |evt: KeyRepeatEvent, _| {
                    inner
                        .lock()
                        .unwrap()
                        .handle_repeat(evt.rawkey, evt.keysym, evt.utf8);
                }
            },
        )
        .map_err(|_| failure::format_err!("Failed to configure keyboard callback"))?;

        Ok(Self { inner })
    }

    pub fn add_window(&self, window_id: usize, surface: &WlSurface) {
        let mut inner = self.inner.lock().unwrap();
        inner
            .surface_to_window_id
            .insert(surface.as_ref().id(), window_id);
    }
}

#[derive(Clone, Debug)]
pub enum KeyboardEvent {
    Enter {
        serial: u32,
    },
    Leave {
        serial: u32,
    },
    Key {
        rawkey: u32,
        keysym: u32,
        is_down: bool,
        serial: u32,
        utf8: Option<String>,
    },
    Modifiers {
        modifiers: Modifiers,
    },
}

impl KeyboardEvent {
    fn from_event(evt: KbEvent) -> Option<Self> {
        Some(match evt {
            KbEvent::Enter { serial, .. } => KeyboardEvent::Enter { serial },
            KbEvent::Leave { serial, .. } => KeyboardEvent::Leave { serial },
            KbEvent::Key {
                rawkey,
                keysym,
                state,
                serial,
                utf8,
                ..
            } => KeyboardEvent::Key {
                rawkey,
                keysym,
                is_down: state == KeyState::Pressed,
                serial,
                utf8,
            },
            KbEvent::Modifiers { modifiers } => KeyboardEvent::Modifiers {
                modifiers: modifier_keys(modifiers),
            },
            _ => return None,
        })
    }
}

fn modifier_keys(state: ModifiersState) -> Modifiers {
    let mut mods = Modifiers::NONE;
    if state.ctrl {
        mods |= Modifiers::CTRL;
    }
    if state.alt {
        mods |= Modifiers::ALT;
    }
    if state.shift {
        mods |= Modifiers::SHIFT;
    }
    if state.logo {
        mods |= Modifiers::SUPER;
    }
    mods
}