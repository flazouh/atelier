use regex::Regex;

/// The screens the app lays out itself.
const SCREENS: [(&str, &str); 9] = [
    ("shell.rs", include_str!("../shell.rs")),
    ("ssh_form.rs", include_str!("../ssh_form.rs")),
    ("ship/strip.rs", include_str!("../ship/strip.rs")),
    ("ship/pull_form.rs", include_str!("../ship/pull_form.rs")),
    ("pull_card.rs", include_str!("../pull_card.rs")),
    ("settings_pane.rs", include_str!("../settings_pane.rs")),
    ("status.rs", include_str!("../status.rs")),
    ("review_pane.rs", include_str!("../review_pane.rs")),
    ("tasks/pane.rs", include_str!("../tasks/pane.rs")),
];

/// Every gap, padding and margin in the screens is on the 4px grid.
#[test]
fn spacing_is_on_the_4px_grid() {
    let spacing = Regex::new(r"\.(gap|gap_x|gap_y|p|px|py|pt|pb|pl|pr|m|mx|my|mt|mb|ml|mr)\(px\((-?[\d.]+)\)\)").unwrap();
    let mut off = Vec::new();
    for (file, source) in SCREENS {
        for (n, line) in source.lines().enumerate() {
            for found in spacing.captures_iter(line) {
                let value: f32 = found[2].parse().unwrap();
                if value % 4. != 0. {
                    off.push(format!("{file}:{}: {}({value})", n + 1, &found[1]));
                }
            }
        }
    }
    assert!(off.is_empty(), "spacing off the 4px grid:\n{}", off.join("\n"));
}

/// Type sizes come from the scale, never a raw `px`.
#[test]
fn type_sizes_come_from_the_scale() {
    let raw = Regex::new(r"text_size\(px\(").unwrap();
    let off: Vec<String> = SCREENS
        .iter()
        .flat_map(|(file, source)| {
            source.lines().enumerate().filter(|(_, l)| raw.is_match(l)).map(move |(n, _)| format!("{file}:{}", n + 1))
        })
        .collect();
    assert!(off.is_empty(), "raw type sizes: {off:?}");
}
