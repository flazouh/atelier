//! What the composer's chips stand for. atelier-ui draws a chip and knows nothing of its meaning: the session keeps the
//! attachment under the chip's id, and reads it back when the message goes.

use std::{collections::HashMap, path::Path, sync::Arc};

use atelier_agents::session::{Attachment, ImageFormat};
use atelier_ui::{Chip, ChipLook, IconName, Message, Pasted};
use gpui_kit::{Context, SharedString};

use super::AgentSession;

/// The most a picture read from a file may weigh: the agent's API refuses much more.
const MOST_IMAGE_BYTES: u64 = 20 * 1024 * 1024;

/// How many characters of pasted text a chip shows on hover.
const PREVIEW_CHARS: usize = 600;

/// A message ready to go: what was written, and what came with it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Draft {
    pub text: String,
    pub attachments: Vec<Attachment>,
}

impl From<String> for Draft {
    fn from(text: String) -> Self {
        Self { text, attachments: Vec::new() }
    }
}

impl From<&str> for Draft {
    fn from(text: &str) -> Self {
        text.to_string().into()
    }
}

impl PartialEq<&str> for Draft {
    fn eq(&self, text: &&str) -> bool {
        self.attachments.is_empty() && self.text == *text
    }
}

/// The attachments the chips over the composer stand for, by chip id.
#[derive(Default)]
pub(super) struct Payloads {
    held: HashMap<SharedString, Attachment>,
    count: u64,
}

impl Payloads {
    fn id(&mut self, kind: &str) -> SharedString {
        self.count += 1;
        format!("{kind}-{}", self.count).into()
    }

    /// The message as a draft: its chips' attachments go with it, and are let go of here. A chip that stands for
    /// nothing but its mention (a file) adds none.
    pub(super) fn draft(&mut self, message: Message) -> Draft {
        let attachments = message.chips.iter().filter_map(|chip| self.held.remove(&chip.id)).collect();
        // The composer let go of every chip when the message went; what is left stood for chips taken off before.
        self.held.clear();
        Draft { text: message.text.to_string(), attachments }
    }
}

/// The chip and the attachment for text that was pasted.
pub(super) fn text_chip(id: SharedString, text: &str) -> (Chip, Attachment) {
    let lines = text.lines().count().max(1);
    let label = if lines == 1 { "Pasted text".to_string() } else { format!("Pasted text · {lines} lines") };
    let preview: String = text.chars().take(PREVIEW_CHARS).collect();
    let preview = if text.chars().count() > PREVIEW_CHARS { format!("{preview}…") } else { preview };
    (Chip::new(id, label).look(ChipLook::Icon(IconName::Draft)).detail(preview), Attachment::Text { text: text.to_string() })
}

/// The chip and the attachment for a picture, if it is in a format the agent takes.
pub(super) fn image_chip(id: SharedString, format: ImageFormat, bytes: Vec<u8>) -> (Chip, Attachment) {
    let kb = bytes.len().div_ceil(1024);
    let thumbnail = Arc::new(gpui_kit::Image::from_bytes(gpui_format(format), bytes.clone()));
    let chip = Chip::new(id, "Image").look(ChipLook::Image(thumbnail)).detail(format!("{} · {kb} KB", format.media_type()));
    (chip, Attachment::Image { format, bytes: bytes.into() })
}

fn gpui_format(format: ImageFormat) -> gpui_kit::ImageFormat {
    match format {
        ImageFormat::Png => gpui_kit::ImageFormat::Png,
        ImageFormat::Jpeg => gpui_kit::ImageFormat::Jpeg,
        ImageFormat::Gif => gpui_kit::ImageFormat::Gif,
        ImageFormat::Webp => gpui_kit::ImageFormat::Webp,
    }
}

/// The formats the agent takes, out of the ones the clipboard offers.
pub(super) fn agent_format(format: gpui_kit::ImageFormat) -> Option<ImageFormat> {
    match format {
        gpui_kit::ImageFormat::Png => Some(ImageFormat::Png),
        gpui_kit::ImageFormat::Jpeg => Some(ImageFormat::Jpeg),
        gpui_kit::ImageFormat::Gif => Some(ImageFormat::Gif),
        gpui_kit::ImageFormat::Webp => Some(ImageFormat::Webp),
        _ => None,
    }
}

/// A line to show in the conversation for what came with a message, if it is not already in the words.
pub fn summary(attachment: &Attachment) -> Option<String> {
    match attachment {
        Attachment::Text { text } => {
            let lines = text.lines().count().max(1);
            Some(if lines == 1 { "Pasted text".to_string() } else { format!("Pasted text · {lines} lines") })
        }
        Attachment::Image { bytes, .. } => Some(format!("Image · {} KB", bytes.len().div_ceil(1024))),
        Attachment::LineComment { .. } | Attachment::File { .. } => None,
    }
}

/// The words, then a line for each thing that came with them.
pub fn shown(text: &str, attachments: &[Attachment]) -> String {
    let lines: Vec<String> = attachments.iter().filter_map(summary).collect();
    match (text.trim().is_empty(), lines.is_empty()) {
        (_, true) => text.to_string(),
        (true, false) => lines.join("\n"),
        (false, false) => format!("{text}\n\n{}", lines.join("\n")),
    }
}

impl AgentSession {
    /// Something was pasted or dropped on the composer: it becomes a chip.
    pub(super) fn pasted(&mut self, pasted: Pasted, cx: &mut Context<Self>) {
        match pasted {
            Pasted::Text(text) => {
                let id = self.payloads.id("text");
                let (chip, attachment) = text_chip(id, &text);
                self.add_chip(chip, Some(attachment), cx);
            }
            Pasted::Image(image) => match agent_format(image.format) {
                Some(format) => {
                    let id = self.payloads.id("image");
                    let (chip, attachment) = image_chip(id, format, image.bytes.clone());
                    self.add_chip(chip, Some(attachment), cx);
                }
                None => self.problem = Some("This picture is in a format the agent cannot read; use PNG, JPEG, GIF or WebP.".into()),
            },
            Pasted::Files(paths) => {
                for path in paths {
                    self.add_file(&path, cx);
                }
            }
        }
        cx.notify();
    }

    /// A file: a picture the agent takes goes as a picture, anything else as a mention of its path.
    fn add_file(&mut self, path: &Path, cx: &mut Context<Self>) {
        let format = path.extension().and_then(|e| e.to_str()).and_then(ImageFormat::of_extension);
        let picture = format.filter(|_| std::fs::metadata(path).is_ok_and(|m| m.len() <= MOST_IMAGE_BYTES));
        if let (Some(format), Some(bytes)) = (picture, picture.and_then(|_| std::fs::read(path).ok())) {
            let id = self.payloads.id("image");
            let (mut chip, attachment) = image_chip(id, format, bytes);
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                chip.label = name.to_string().into();
            }
            return self.add_chip(chip, Some(attachment), cx);
        }
        let shown = path.strip_prefix(self.project.root()).unwrap_or(path);
        self.add_chip(Chip::file(shown.to_string_lossy().into_owned()), None, cx);
    }

    fn add_chip(&mut self, chip: Chip, attachment: Option<Attachment>, cx: &mut Context<Self>) {
        if let Some(attachment) = attachment {
            self.payloads.held.insert(chip.id.clone(), attachment);
        }
        self.composer.update(cx, |c, cx| c.add_chip(chip, cx));
    }

    /// The composer's message as a draft; what its chips stood for goes with it.
    pub(super) fn draft_of(&mut self, message: Message) -> Draft {
        self.payloads.draft(message)
    }
}
