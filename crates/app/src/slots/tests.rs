use atelier_ui::{IconName, Work};

use super::{Column, RailView, Slots, StatusBarCard, builtin};
use crate::vitals::{SysinfoProbe, Vitals};

fn card(id: &'static str, column: Column, order: i32) -> StatusBarCard {
    StatusBarCard::new(id, column, order, |_, _, _| Vec::new())
}

fn vitals() -> Vitals {
    Vitals::new(Box::new(SysinfoProbe::new()))
}

fn ids(slots: &Slots, column: Column, vitals: &Vitals) -> Vec<String> {
    slots
        .cards(column, vitals)
        .iter()
        .map(|card| card.id.to_string())
        .collect()
}

fn view(id: &'static str, order: i32) -> RailView {
    RailView::new(id, Some(IconName::Code), id, order, |_, _| {
        unreachable!("not opened")
    })
}

#[test]
fn cards_of_a_column_stand_by_order_then_by_id() {
    let mut slots = Slots::default();
    slots.add_card(card("b", Column::Middle, 2));
    slots.add_card(card("a", Column::Middle, 2));
    slots.add_card(card("c", Column::Middle, 1));
    slots.add_card(card("z", Column::Left, 0));
    assert_eq!(ids(&slots, Column::Middle, &vitals()), ["c", "a", "b"]);
    assert_eq!(ids(&slots, Column::Left, &vitals()), ["z"]);
    assert!(ids(&slots, Column::Right, &vitals()).is_empty());
}

#[test]
fn a_card_that_is_not_visible_is_not_drawn() {
    let mut slots = Slots::default();
    slots.add_card(card("shown", Column::Middle, 0));
    slots.add_card(card("hidden", Column::Middle, 1).when(|_| false));
    assert_eq!(ids(&slots, Column::Middle, &vitals()), ["shown"]);
}

#[test]
fn a_card_is_visible_by_the_state_of_the_bar() {
    let mut slots = Slots::default();
    slots.add_card(card("machine", Column::Right, 0).when(|vitals| vitals.load().is_some()));
    let mut vitals = vitals();
    assert!(
        ids(&slots, Column::Right, &vitals).is_empty(),
        "no sample yet"
    );
    vitals.sample();
    assert_eq!(ids(&slots, Column::Right, &vitals), ["machine"]);
}

#[test]
fn a_card_added_with_the_id_of_another_replaces_it() {
    let mut slots = Slots::default();
    slots.add_card(card("a", Column::Left, 5));
    slots.add_card(card("a", Column::Middle, 1));
    assert!(
        ids(&slots, Column::Left, &vitals()).is_empty(),
        "the first is gone"
    );
    assert_eq!(ids(&slots, Column::Middle, &vitals()), ["a"]);
}

#[test]
fn a_view_added_with_the_id_of_another_replaces_it_and_views_stand_by_order() {
    let mut slots = Slots::default();
    slots.add_view(view("b", 2));
    slots.add_view(view("a", 3));
    slots.add_view(view("b", 5));
    let order: Vec<_> = slots
        .views()
        .iter()
        .map(|view| view.id.to_string())
        .collect();
    assert_eq!(order, ["a", "b"]);
    assert_eq!(slots.view("b").map(|view| view.order), Some(5));
    assert!(slots.view("nobody").is_none());
}

#[test]
fn the_built_in_cards_stand_where_the_bar_had_them() {
    let slots = builtin();
    let mut vitals = vitals();
    vitals.sample();
    vitals.set_work(Work::new(0, 2));
    assert_eq!(ids(&slots, Column::Left, &vitals), ["version"]);
    assert_eq!(ids(&slots, Column::Middle, &vitals), ["work", "usage"]);
    assert_eq!(ids(&slots, Column::Right, &vitals), ["system"]);
    vitals.set_work(Work::new(2, 0));
    assert_eq!(
        ids(&slots, Column::Middle, &vitals),
        ["usage"],
        "work that needs no one is not told"
    );
}

#[test]
fn without_the_usage_module_the_bar_has_no_usage_card_and_no_usage_view() {
    let mut slots = Slots::default();
    crate::changelog::register(&mut slots);
    crate::vitals::register(&mut slots);
    let mut vitals = vitals();
    vitals.sample();
    assert!(
        ids(&slots, Column::Middle, &vitals)
            .iter()
            .all(|id| id != "usage")
    );
    assert!(slots.view("usage").is_none());
    assert!(builtin().view("usage").is_some());
}
