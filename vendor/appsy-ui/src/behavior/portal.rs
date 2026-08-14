//! Portal — render children into a host element appended to `document.body`,
//! outside the parent DOM tree. Replaces `@radix-ui/react-portal`.
//!
//! # Spec (state machine)
//!
//! States: `Idle` (no host in the document) · `Mounted` (host div appended to
//! body, children mounted inside it).
//!
//! | State   | Event      | Next    | Effect        |
//! |---------|------------|---------|---------------|
//! | Idle    | `Activate` | Mounted | `MountHost`   |
//! | Mounted | `Cleanup`  | Idle    | `UnmountHost` |
//! | Mounted | `Activate` | Mounted | none          |
//! | Idle    | `Cleanup`  | Idle    | none          |
//!
//! Invariants: at most one host per portal instance; the host is removed
//! exactly once; on the server the portal renders nothing into the main tree
//! (Radix portals only exist after client mount — hydration finds no
//! serialized portal content, and the client effect mounts it fresh).
//!
//! Keyboard map: none (infrastructure primitive; no APG pattern).

use leptos::prelude::*;

/// Pure transition function — the tested machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PortalState {
    Idle,
    Mounted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PortalEvent {
    Activate,
    Cleanup,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PortalEffect {
    MountHost,
    UnmountHost,
    None,
}

pub fn transition(state: PortalState, event: PortalEvent) -> (PortalState, PortalEffect) {
    match (state, event) {
        (PortalState::Idle, PortalEvent::Activate) => (PortalState::Mounted, PortalEffect::MountHost),
        (PortalState::Mounted, PortalEvent::Cleanup) => (PortalState::Idle, PortalEffect::UnmountHost),
        (PortalState::Mounted, PortalEvent::Activate) => (PortalState::Mounted, PortalEffect::None),
        (PortalState::Idle, PortalEvent::Cleanup) => (PortalState::Idle, PortalEffect::None),
    }
}

/// Teleport `children` into a `div` host appended to `document.body`.
/// Renders nothing in place; the host mounts in a client effect and unmounts
/// (host removed) when the owning scope is disposed.
#[component]
pub fn Portal(children: ChildrenFn) -> impl IntoView {
    let state = std::rc::Rc::new(std::cell::Cell::new(PortalState::Idle));

    #[cfg(any(feature = "csr", feature = "hydrate"))]
    {
        use std::any::Any;
        use std::cell::RefCell;
        use std::rc::Rc;
        use wasm_bindgen::JsCast;

        let mount: Rc<RefCell<Option<(web_sys::Element, Box<dyn Any>)>>> =
            Rc::new(RefCell::new(None));

        Effect::new({
            let mount = Rc::clone(&mount);
            let state = Rc::clone(&state);
            move |_| {
                let (next, effect) = transition(state.get(), PortalEvent::Activate);
                state.set(next);
                if effect != PortalEffect::MountHost {
                    return;
                }
                let document = leptos::tachys::dom::document();
                let host = document.create_element("div").expect("invariant: create portal host");
                document
                    .body()
                    .expect("invariant: document has body")
                    .append_child(&host)
                    .expect("invariant: append portal host");
                let children = children.clone();
                let handle = leptos::mount::mount_to(
                    host.clone().unchecked_into(),
                    move || children(),
                );
                *mount.borrow_mut() = Some((host, Box::new(handle)));
            }
        });

        on_cleanup({
            let cleanup = send_wrapper::SendWrapper::new((Rc::clone(&mount), Rc::clone(&state)));
            move || {
                let (mount, state) = &*cleanup;
                let (next, effect) = transition(state.get(), PortalEvent::Cleanup);
                state.set(next);
                if effect != PortalEffect::UnmountHost {
                    return;
                }
                if let Some((host, handle)) = mount.borrow_mut().take() {
                    drop(handle); // unmounts the leptos root
                    host.remove();
                }
            }
        });
    }

    #[cfg(not(any(feature = "csr", feature = "hydrate")))]
    {
        let _ = (children, state);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn machine_covers_every_transition() {
        assert_eq!(
            transition(PortalState::Idle, PortalEvent::Activate),
            (PortalState::Mounted, PortalEffect::MountHost)
        );
        assert_eq!(
            transition(PortalState::Mounted, PortalEvent::Cleanup),
            (PortalState::Idle, PortalEffect::UnmountHost)
        );
        assert_eq!(
            transition(PortalState::Mounted, PortalEvent::Activate),
            (PortalState::Mounted, PortalEffect::None)
        );
        assert_eq!(
            transition(PortalState::Idle, PortalEvent::Cleanup),
            (PortalState::Idle, PortalEffect::None)
        );
    }

    /// The invariant the machine enforces: a full lifecycle mounts once and
    /// unmounts once, no matter how many times events repeat.
    #[test]
    fn repeated_events_mount_and_unmount_exactly_once() {
        let mut state = PortalState::Idle;
        let mut mounts = 0;
        let mut unmounts = 0;
        for event in [
            PortalEvent::Activate,
            PortalEvent::Activate,
            PortalEvent::Cleanup,
            PortalEvent::Cleanup,
        ] {
            let (next, effect) = transition(state, event);
            state = next;
            match effect {
                PortalEffect::MountHost => mounts += 1,
                PortalEffect::UnmountHost => unmounts += 1,
                PortalEffect::None => {}
            }
        }
        assert_eq!((mounts, unmounts), (1, 1));
        assert_eq!(state, PortalState::Idle);
    }
}
