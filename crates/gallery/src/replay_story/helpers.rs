use atelier_ui::{ToolCall as ToolRow, ToolStatus as RowToolStatus};
use gpui_kit::SharedString;
use atelier_agents::session::{Call, ToolStatus};

fn row_status(status: ToolStatus) -> RowToolStatus {
    match status {
        ToolStatus::Pending | ToolStatus::Running => RowToolStatus::Running,
        ToolStatus::Done => RowToolStatus::Done,
        ToolStatus::Failed => RowToolStatus::Failed,
    }
}

pub(super) fn tool_row(id: impl Into<gpui_kit::ElementId>, call: &Call) -> ToolRow {
    let mut row = ToolRow::new(id, SharedString::from(call.call.name.clone())).status(row_status(call.call.status));
    if let Some(file) = &call.call.file {
        row = row.file(file.clone());
    }
    if let Some(output) = &call.output {
        let note = if output.truncated { "\n… cut, the rest is kept by the agent" } else { "" };
        row = row.output(format!("{}{note}", output.text));
    }
    row
}
