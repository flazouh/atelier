use atelier_ui::{
    button::{Button, ButtonVariant},
    menu::{self, Entry, Menu, MenuLook, Origin, Pick, entries_of},
    popover::{Hang, Popover},
};
use gpui_kit::{App, Entity, IntoElement, ParentElement, Styled, Window, div};

use crate::agent_session::{AgentSession, SessionEvent};

/// Space between the button and the menu that opens above it.
const MENU_GAP: f32 = 4.;
/// The button's words.
const HANDOFF: &str = "Handoff";

/// The button that opens the handoff menu over a notice: the agents, and for an agent with a choice its providers.
/// A choice hands the session off at once. `name` is the button's debug name, and the base of its ids.
pub(in crate::session_view) fn handoff_button(session: &Entity<AgentSession>, name: &'static str, window: &mut Window, cx: &mut App) -> impl IntoElement + use<> {
    let s = session.read(cx);
    let (key, branches) = (s.key.clone(), s.handoff_branches.clone());
    let open = window.use_keyed_state(gpui_kit::ElementId::Name(format!("{key}-{name}-open").into()), cx, |_, _| false);
    let is_open = *open.read(cx) && !branches.is_empty();
    let shut = {
        let open = open.clone();
        move |cx: &mut App| open.update(cx, |o, cx| {
            *o = false;
            cx.notify();
        })
    };
    let menu = is_open.then(|| {
        let (owner, closing) = (session.clone(), shut.clone());
        // A choice shuts the menu, then asks for the handoff.
        let pick: Pick = std::rc::Rc::new(move |target, _, cx| {
            closing(cx);
            owner.update(cx, |_, cx| cx.emit(SessionEvent::Handoff(target.clone())));
        });
        let entries: Vec<Entry> = entries_of(&branches, &pick);
        let rows = entries.len();
        let close = shut.clone();
        Popover::new(gpui_kit::ElementId::Name(format!("{key}-{name}-popover").into()))
            .open(true)
            .hang(Hang::Right(0., -(menu::height_in(MenuLook::PROJECT, rows) + MENU_GAP)))
            .keep_focus()
            .height(menu::height_in(MenuLook::PROJECT, rows))
            .on_close(move |_, cx| close(cx))
            .child(Menu::new(gpui_kit::ElementId::Name(format!("{key}-{name}-menu").into()), entries).look(MenuLook::PROJECT).origin(Origin::BottomRight))
    });
    let toggle = open.clone();
    div()
        .relative()
        .child(
            Button::new(gpui_kit::ElementId::Name(format!("{key}-{name}").into()))
                .label(HANDOFF)
                .variant(ButtonVariant::Secondary)
                .debug_name(name)
                .open(is_open)
                .on_click(move |_, _, cx| {
                    toggle.update(cx, |o, cx| {
                        *o = !*o;
                        cx.notify();
                    })
                }),
        )
        .children(menu)
}
