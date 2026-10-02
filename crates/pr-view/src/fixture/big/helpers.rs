pub(super) fn text(file: usize, edited: bool) -> String {
    (0..40).map(|l| if edited && l == 20 { format!("fn f{file}_{l}() {{ changed() }}\n") } else { format!("fn f{file}_{l}() {{}}\n") }).collect()
}

pub fn path(file: usize) -> String {
    format!("src/m{}/f{file}.rs", file % 20)
}
