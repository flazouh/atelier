use super::types::Attachment;

/// `text`, then each attachment, apart by a blank line.
pub fn message_text(text: &str, attachments: &[Attachment]) -> String {
    let mut out = text.to_string();
    for attachment in attachments {
        out.push_str("\n\n");
        out.push_str(&attachment.render());
    }
    out
}
