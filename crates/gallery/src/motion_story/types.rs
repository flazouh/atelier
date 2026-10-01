/// The demo's accents, from `color-selector.preview.tsx`, as red, green and blue bytes: they are the
/// user's data, not the UI's colours.
pub(super) const ACCENTS: [(&str, [u8; 3], &str); 8] = [
    ("blue", [52, 120, 246], "Blue"),
    ("purple", [146, 112, 232], "Purple"),
    ("pink", [230, 106, 164], "Pink"),
    ("red", [229, 86, 86], "Red"),
    ("orange", [237, 145, 65], "Orange"),
    ("amber", [229, 182, 60], "Amber"),
    ("green", [101, 166, 90], "Green"),
    ("teal", [22, 157, 131], "Teal"),
];
