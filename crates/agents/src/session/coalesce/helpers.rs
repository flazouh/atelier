use super::super::event::Event;

pub(super) fn join_or_push(pending: &mut Vec<Event>, event: Event) {
    match (pending.last_mut(), event) {
        (Some(Event::Text { block, delta }), Event::Text { block: next, delta: more }) if *block == next => {
            delta.push_str(&more)
        }
        (Some(Event::Thinking { block, delta }), Event::Thinking { block: next, delta: more }) if *block == next => {
            delta.push_str(&more)
        }
        (Some(Event::ToolEdit { id, edit }), Event::ToolEdit { id: next, edit: newer }) if *id == next => *edit = newer,
        (_, event) => pending.push(event),
    }
}
