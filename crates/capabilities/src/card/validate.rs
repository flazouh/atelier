use super::{
    resolve::LIST_MAX,
    structs::{Card, Node},
};

const DEPTH_MAX: usize = 6;
const NODES_MAX: usize = 200;
const CHILDREN_MAX: usize = 12;
const FOOTER_MAX: usize = 3;
const TEXT_LINES_MAX: u8 = 6;
const CODE_LINES_MAX: u8 = 30;

/// Reads a card from JSON and checks its limits. A plugin's card goes through this when the plugin installs.
pub fn from_json(json: &str) -> Result<Card, String> {
    let card: Card =
        serde_json::from_str(json).map_err(|e| format!("the card does not parse: {e}"))?;
    validate(&card).map_err(|problems| problems.join("; "))?;
    Ok(card)
}

/// The limits of `docs/capabilities/card-schema-v1.md` section 5. Every problem is listed.
pub fn validate(card: &Card) -> Result<(), Vec<String>> {
    let mut problems = Vec::new();
    if card.version != 1 {
        problems.push(format!("version {} is not known", card.version));
    }
    if card.tool.trim().is_empty() {
        problems.push("the tool name is empty".into());
    }
    if card.done.footer.len() > FOOTER_MAX {
        problems.push(format!(
            "the footer has {} actions, the most is {FOOTER_MAX}",
            card.done.footer.len()
        ));
    }
    let mut count = 0;
    if let Some(body) = &card.done.body {
        walk(body, 1, &mut count, &mut problems);
    }
    if count > NODES_MAX {
        problems.push(format!(
            "the card has {count} nodes, the most is {NODES_MAX}"
        ));
    }
    if problems.is_empty() {
        Ok(())
    } else {
        Err(problems)
    }
}

fn walk(node: &Node, depth: usize, count: &mut usize, problems: &mut Vec<String>) {
    *count += 1;
    if depth > DEPTH_MAX {
        problems.push(format!(
            "a node is {depth} levels deep, the most is {DEPTH_MAX}"
        ));
        return;
    }
    match node {
        Node::Stack { children, .. } => {
            if children.len() > CHILDREN_MAX {
                problems.push(format!(
                    "a stack has {} children, the most is {CHILDREN_MAX}",
                    children.len()
                ));
            }
            children
                .iter()
                .for_each(|c| walk(c, depth + 1, count, problems));
        }
        Node::List { item, limit, .. } => {
            if limit.is_some_and(|n| usize::from(n) > LIST_MAX) {
                problems.push(format!("a list shows at most {LIST_MAX} rows"));
            }
            walk(item, depth + 1, count, problems);
        }
        Node::Text {
            max_lines: Some(n), ..
        } if *n > TEXT_LINES_MAX => {
            problems.push(format!("text shows at most {TEXT_LINES_MAX} lines"))
        }
        Node::Code {
            max_lines: Some(n), ..
        } if *n > CODE_LINES_MAX => {
            problems.push(format!("code shows at most {CODE_LINES_MAX} lines"))
        }
        _ => {}
    }
}
