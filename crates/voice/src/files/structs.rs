/// One file of the model: the name it has in the repository, the name the loader looks for, its size and its SHA-256.
#[derive(Clone, Copy, Debug)]
pub struct ModelFile {
    pub remote: &'static str,
    pub local: &'static str,
    pub size: u64,
    pub sha256: &'static str,
}
