//! A pull request kept on disk, so the next open draws from it at once and the forge only brings what
//! changed. One JSON file per pull request in the app's data folder, written whole through a temporary
//! name. A file that does not read (a newer or older lathe wrote it, or the disk lost a byte) is a miss,
//! never an error.
use std::{
    fs, io,
    path::{Path, PathBuf},
};

use lathe_forge::PullRef;

use crate::data::PullData;

/// Bump when the shape of [`PullData`] changes; older files are then ignored.
const VERSION: u32 = 1;

#[derive(serde::Serialize, serde::Deserialize)]
struct Envelope {
    version: u32,
    data: PullData,
}

pub struct Snapshots {
    dir: PathBuf,
}

impl Snapshots {
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self { dir: dir.into() }
    }

    /// `github.com-oven-sh-bun-44169.json`, with anything but letters, digits, dots and dashes made a dash.
    fn file(&self, reference: &PullRef) -> PathBuf {
        let name = format!("{}-{}-{}-{}.json", reference.repo.host, reference.repo.owner, reference.repo.name, reference.number);
        let safe: String = name.chars().map(|c| if c.is_ascii_alphanumeric() || c == '.' || c == '-' { c } else { '-' }).collect();
        self.dir.join(safe)
    }

    pub fn load(&self, reference: &PullRef) -> Option<PullData> {
        let bytes = fs::read(self.file(reference)).ok()?;
        let envelope: Envelope = serde_json::from_slice(&bytes).ok()?;
        (envelope.version == VERSION && envelope.data.reference == *reference).then_some(envelope.data)
    }

    pub fn save(&self, data: &PullData) -> io::Result<()> {
        fs::create_dir_all(&self.dir)?;
        let target = self.file(&data.reference);
        let temporary = target.with_extension("json.saving");
        let bytes = serde_json::to_vec(&Envelope { version: VERSION, data: data.clone() }).map_err(io::Error::other)?;
        fs::write(&temporary, bytes)?;
        fs::rename(&temporary, &target)
    }

    /// Forgets a pull request, when it is closed and its checkout goes.
    pub fn remove(&self, reference: &PullRef) {
        let _ = fs::remove_file(self.file(reference));
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }
}
