use atelier_ui::{MultiOption, Swatch};
use gpui_kit::{AnyElement, Hsla, IntoElement, ParentElement, Rgba, Styled, div, px};

use super::types::ACCENTS;

/// The demo's teams, from `multi-select.preview.tsx`; the dots are Tailwind's rose, sky, amber, violet, emerald and slate 500.
pub(super) fn teams() -> Vec<MultiOption> {
    let dot = |r: u8, g: u8, b: u8| Hsla::from(Rgba { r: r as f32 / 255., g: g as f32 / 255., b: b as f32 / 255., a: 1. });
    let product = "Product teams";
    let business = "Business teams";
    vec![
        MultiOption::new("design", "Design").group(product).dot(dot(244, 63, 94)),
        MultiOption::new("engineering", "Engineering").group(product).dot(dot(14, 165, 233)),
        MultiOption::new("product", "Product").group(product).dot(dot(245, 158, 11)),
        MultiOption::new("research", "Research").group(product).dot(dot(139, 92, 246)),
        MultiOption::new("marketing", "Marketing").group(business).dot(dot(16, 185, 129)),
        MultiOption::new("operations", "Operations").group(business).dot(dot(100, 116, 139)),
    ]
}

/// The web preview's queue: one arrived, one on its way, one that failed.
pub(super) fn initial_uploads() -> Vec<atelier_ui::UploadItem> {
    vec![
        atelier_ui::UploadItem::new("brand-assets", "brand-assets.zip", 18_400_000).mime("application/zip").progress(100.).status(atelier_ui::UploadStatus::Success),
        atelier_ui::UploadItem::new("release-video", "release-cut.mov", 84_200_000).mime("video/quicktime").progress(58.).status(atelier_ui::UploadStatus::Uploading),
        atelier_ui::UploadItem::new("contracts", "vendor-contract.pdf", 2_800_000).mime("application/pdf").progress(32.).status(atelier_ui::UploadStatus::Error).error("Connection lost"),
    ]
}

pub(super) fn accents() -> Vec<Swatch> {
    ACCENTS
        .iter()
        .map(|(value, [r, g, b], label)| {
            let color = Hsla::from(Rgba { r: *r as f32 / 255., g: *g as f32 / 255., b: *b as f32 / 255., a: 1. });
            Swatch::new(*value, color, *label)
        })
        .collect()
}

pub(super) fn section(title: &'static str, theme: &atelier_ui::Theme, body: impl IntoElement) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .gap(px(10.))
        .child(div().text_size(px(12.)).text_color(theme.muted_foreground).child(title))
        .child(body)
        .into_any_element()
}
