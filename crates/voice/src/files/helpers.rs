use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    time::Duration,
};

use sha2::{Digest, Sha256};

use crate::Error;
use super::structs::ModelFile;

/// Every byte of the model.
pub fn total_bytes(files: &[ModelFile]) -> u64 {
    files.iter().map(|f| f.size).sum()
}

/// `<data>/atelier/speech/parakeet-tdt-v2`, where the model lives on this machine.
pub fn dir() -> Option<PathBuf> {
    Some(dirs::data_dir()?.join("atelier").join("speech").join("parakeet-tdt-v2"))
}

/// The folder an earlier model used, `<data>/atelier/speech/phonon-2`. It holds about 735 MB nobody uses any more.
pub fn legacy_dir() -> Option<PathBuf> {
    Some(dirs::data_dir()?.join("atelier").join("speech").join("phonon-2"))
}

/// Whether every file is there at its pinned size. They are hashed when they arrive, so the size is enough to ask later.
pub fn installed(dir: &Path, files: &[ModelFile]) -> bool {
    files.iter().all(|f| fs::metadata(dir.join(f.local)).is_ok_and(|m| m.len() == f.size))
}

/// Fetches what is missing into `dir`, from `base`, and tells `progress` how many bytes of the whole are in place. A file is
/// written as `<name>.part`, checked against its hash, and only then renamed, so a stopped download never looks installed; the
/// next call goes on from the bytes the `.part` holds.
pub fn install(base: &str, dir: &Path, files: &[ModelFile], progress: &mut dyn FnMut(u64, u64)) -> Result<(), Error> {
    fs::create_dir_all(dir)?;
    let total = total_bytes(files);
    let mut done = 0u64;
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_connect(Some(Duration::from_secs(15)))
        .timeout_recv_body(Some(Duration::from_secs(30)))
        .build()
        .into();
    for file in files {
        let target = dir.join(file.local);
        if fs::metadata(&target).is_ok_and(|m| m.len() == file.size) {
            done += file.size;
            progress(done, total);
            continue;
        }
        let part = dir.join(format!("{}.part", file.local));
        // A download that stopped part way goes on from where it got to: the bytes kept are hashed again, and the rest asked for.
        let mut hash = Sha256::new();
        let mut written = match fs::metadata(&part) {
            Ok(m) if m.len() > 0 && m.len() < file.size => {
                let mut kept = fs::File::open(&part)?;
                let mut buffer = vec![0u8; 1024 * 1024];
                loop {
                    let n = kept.read(&mut buffer)?;
                    if n == 0 {
                        break;
                    }
                    hash.update(&buffer[..n]);
                }
                m.len()
            }
            _ => 0,
        };
        let mut request = agent.get(&format!("{base}/{}", file.remote));
        if written > 0 {
            request = request.header("Range", &format!("bytes={written}-"));
        }
        let mut response = request.call().map_err(|why| Error::Network(why.to_string()))?;
        let mut out = if written > 0 && response.status().as_u16() == 206 {
            fs::OpenOptions::new().append(true).open(&part)?
        } else {
            // The server sent the whole file: start again.
            written = 0;
            hash = Sha256::new();
            fs::File::create(&part)?
        };
        progress(done + written, total);
        let mut reader = response.body_mut().as_reader();
        let mut buffer = vec![0u8; 256 * 1024];
        loop {
            let n = reader.read(&mut buffer).map_err(|why| Error::Network(why.to_string()))?;
            if n == 0 {
                break;
            }
            out.write_all(&buffer[..n])?;
            hash.update(&buffer[..n]);
            written += n as u64;
            progress((done + written).min(total), total);
        }
        out.flush()?;
        drop(out);
        if written != file.size || hex(&hash.finalize()) != file.sha256 {
            let _ = fs::remove_file(&part);
            return Err(Error::Checksum(file.local));
        }
        fs::rename(&part, &target)?;
        done += file.size;
    }
    progress(total, total);
    Ok(())
}

pub(super) fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
