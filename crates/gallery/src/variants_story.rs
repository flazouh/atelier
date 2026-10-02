//! The "Variants" story: designs for three controls, each in atelier's look (our tokens, sizes and radii,
//! borderless) with the motion running, so Alex can pick. The toggle and tab designs are the parts the real app
//! shows through Settings, "Design preview" (`atelier_ui::design_preview`): there is no second copy.
//!
//! 1. The spinner: A the old glyph, B the ring, C three dots, D a bar sweep. Each 14px, at the text size.
//! 2. The editor tabs: A a chip, B a chip with a 2px bottom line, C a text tab with an under-dot, D a tab with a
//!    left accent tick. The marker glides between the tabs on the tabs' own spring.
//!
//! `VARIANTS_GROUP=spinner|tabs` shows one group alone, for a screenshot.

mod helpers;
mod structs;

pub use structs::VariantsStory;
