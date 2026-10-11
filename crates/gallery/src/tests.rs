use gpui_kit::{Entity, Modifiers, TestAppContext, VisualTestContext, px, size};

use super::{groups::Group, *};

fn open(cx: &mut TestAppContext) -> (Entity<Gallery>, &mut VisualTestContext) {
    cx.update(|cx| {
        atelier_ui::init(cx);
        atelier_ui::theme::set_theme(atelier_ui::themes::atelier(Appearance::Dark).clone(), cx);
        cx.set_reduce_motion(true);
    });
    let (gallery, cx) = cx.add_window_view(Gallery::new);
    cx.simulate_resize(size(px(1100.), px(860.)));
    cx.run_until_parked();
    (gallery, cx)
}

fn type_in_search(gallery: &Entity<Gallery>, cx: &mut VisualTestContext, words: &'static str) {
    gallery.update_in(cx, |g, window, cx| g.search.clone().update(cx, |search, cx| search.set_value(words, window, cx)));
    cx.run_until_parked();
}

#[test]
fn every_story_is_in_one_group_and_no_group_is_empty() {
    let sizes: Vec<usize> = Group::ALL.into_iter().map(|group| group.stories().count()).collect();
    assert_eq!(sizes.iter().sum::<usize>(), Story::ALL.len(), "{sizes:?}");
    assert!(sizes.iter().all(|size| *size > 0), "{sizes:?}");
    assert_eq!(Story::Buttons.group(), Group::Controls);
}

#[gpui_kit::test]
fn the_sidebar_starts_with_six_folded_groups_and_only_the_one_of_the_story_in_front_open(cx: &mut TestAppContext) {
    let (gallery, cx) = open(cx);
    for group in Group::ALL {
        assert!(cx.debug_bounds(Box::leak(format!("nav-group-{}", group.name()).into_boxed_str())).is_some(), "{group:?} has its head");
    }
    assert_eq!(gallery.read_with(cx, |g, _| g.story.group()), Group::Agent);
    assert!(cx.debug_bounds("nav-row-Agent panel").is_some(), "the story in front is listed");
    assert!(cx.debug_bounds("nav-row-Buttons").is_none(), "a folded group lists nothing");
    let last = cx.debug_bounds("nav-group-App").unwrap();
    let picker = cx.debug_bounds("theme-picker").unwrap();
    assert!(last.bottom() <= picker.top(), "the whole list fits over the theme picker, with no scroll");
}

#[gpui_kit::test]
fn a_press_on_a_group_opens_it_a_press_on_a_row_shows_its_story_and_a_second_press_folds_the_group(cx: &mut TestAppContext) {
    let (gallery, cx) = open(cx);
    let head = cx.debug_bounds("nav-group-Controls").unwrap();
    cx.simulate_click(head.center(), Modifiers::default());
    cx.run_until_parked();
    let buttons = cx.debug_bounds("nav-row-Buttons").expect("the group is open");
    cx.simulate_click(buttons.center(), Modifiers::default());
    cx.run_until_parked();
    assert!(gallery.read_with(cx, |g, _| g.story == Story::Buttons));
    let head = cx.debug_bounds("nav-group-Controls").unwrap();
    cx.simulate_click(head.center(), Modifiers::default());
    cx.run_until_parked();
    assert!(cx.debug_bounds("nav-row-Buttons").is_none(), "folded again");
    assert!(gallery.read_with(cx, |g, _| g.story == Story::Buttons), "the story stays in front");
}

#[gpui_kit::test]
fn letters_in_the_search_list_the_stories_that_match_with_no_groups_and_enter_opens_the_first(cx: &mut TestAppContext) {
    let (gallery, cx) = open(cx);
    assert!(cx.debug_bounds("stories-search").is_some(), "the search field is drawn");
    type_in_search(&gallery, cx, "but");
    assert!(cx.debug_bounds("nav-group-Controls").is_none(), "no groups while searching");
    let buttons = cx.debug_bounds("nav-row-Buttons").expect("Buttons matches");
    assert_eq!(groups::found("but").first(), Some(&Story::Buttons), "and it is the best match");
    assert!(cx.debug_bounds("nav-row-Colors").is_none(), "a story that does not match is not listed");
    assert!(buttons.top() < cx.debug_bounds("theme-picker").unwrap().top());
    gallery.update_in(cx, |g, window, cx| g.open_first_found(window, cx));
    cx.run_until_parked();
    assert!(gallery.read_with(cx, |g, cx| g.story == Story::Buttons && g.search.read(cx).value().is_empty()), "the story opens and the field empties");
    assert!(cx.debug_bounds("nav-group-Controls").is_some() && cx.debug_bounds("nav-row-Buttons").is_some(), "the groups are back, with its group open");
}

#[gpui_kit::test]
fn letters_that_match_no_story_say_so(cx: &mut TestAppContext) {
    let (gallery, cx) = open(cx);
    type_in_search(&gallery, cx, "zzqx");
    assert!(cx.debug_bounds("stories-none").is_some());
    gallery.update_in(cx, |g, window, cx| g.open_first_found(window, cx));
    assert!(gallery.read_with(cx, |g, _| g.story == Story::AgentPanel), "Enter with nothing listed changes nothing");
}

/// The sidebar lists more stories than fit in an 1100 by 860 window: the list scrolls, and the theme
/// picker at its foot stays whole in the window.
#[gpui_kit::test]
fn the_theme_picker_stays_in_a_short_window(cx: &mut TestAppContext) {
    cx.update(|cx| {
        atelier_ui::init(cx);
        atelier_ui::theme::set_theme(atelier_ui::themes::atelier(Appearance::Dark).clone(), cx);
    });
    let (_gallery, cx) = cx.add_window_view(Gallery::new);
    cx.simulate_resize(size(px(1100.), px(860.)));
    cx.run_until_parked();
    let picker = cx.debug_bounds("theme-picker").expect("the picker is drawn");
    let window = cx.update(|window, _| window.viewport_size());
    assert!(picker.bottom() <= window.height, "the picker ends at {:?}, in a window {:?} tall", picker.bottom(), window.height);
    assert!(picker.top() >= px(0.));
}
