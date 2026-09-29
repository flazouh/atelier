use lathe_project::Query;
use serde_json::{Value, json};

use super::{Access, Tool, ToolContext, ToolResult, cap, object, MAX_RESULT};
use crate::session::ToolKind;

pub struct Search;

impl Tool for Search {
    fn name(&self) -> &'static str {
        "search"
    }

    fn description(&self) -> &'static str {
        "Searches the text of the project's files. Returns matching lines as `path:line: text`. Case is ignored unless `case_sensitive` is set."
    }

    fn schema(&self) -> Value {
        object(
            json!({
                "pattern": {"type": "string", "description": "The text to find, or a regular expression when `regex` is set."},
                "regex": {"type": "boolean", "description": "Read the pattern as a regular expression."},
                "case_sensitive": {"type": "boolean"},
                "limit": {"type": "integer", "description": "The most matching lines to return. 100 by default, 500 at most."}
            }),
            &["pattern"],
        )
    }

    fn kind(&self) -> ToolKind {
        ToolKind::Search
    }

    fn access(&self) -> Access {
        Access::Read
    }

    fn file(&self, _: &Value) -> Option<String> {
        None
    }

    fn run(&self, ctx: &ToolContext<'_>, input: &Value) -> ToolResult {
        let Some(pattern) = input["pattern"].as_str().filter(|p| !p.is_empty()) else { return ToolResult::err("`pattern` is required") };
        let query = Query {
            pattern: pattern.to_string(),
            regex: input["regex"].as_bool().unwrap_or(false),
            case_sensitive: input["case_sensitive"].as_bool().unwrap_or(false),
            limit: input["limit"].as_u64().map_or(100, |l| (l as usize).clamp(1, 500)),
        };
        match ctx.project.search(&query) {
            Ok(matches) if matches.is_empty() => ToolResult::ok("No matches"),
            Ok(matches) => {
                let mut out = String::new();
                for m in &matches {
                    let text: String = m.text.trim_end().chars().take(300).collect();
                    out.push_str(&format!("{}:{}: {}\n", m.path, m.line + 1, text));
                }
                if matches.len() >= query.limit {
                    out.push_str(&format!("[stopped at {} lines; narrow the pattern to see others]\n", query.limit));
                }
                ToolResult::ok(cap(&out, MAX_RESULT))
            }
            Err(e) => ToolResult::err(format!("the search failed: {e}")),
        }
    }
}
