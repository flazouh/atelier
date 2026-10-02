//! The model's files: Phonon-2 by Fermion Research (CC-BY-4.0 weights), exported to ONNX by tiyuvta, pinned to one commit and
//! to the hash of every file, so what runs is what was reviewed. The encoder is the `exact4x2` one: the same tokens as the
//! fp32 model, a 662 MB file that needs 1.1 GB of memory instead of 2.2.
use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    time::Duration,
};

use sha2::{Digest, Sha256};

use crate::Error;

/// Where the files are fetched from, pinned to a commit.
pub const BASE: &str = "https://huggingface.co/tiyuvta/Phonon-2-ONNX/resolve/12c9688bbc4fc52d23c1a66ca873fd3ac6ed4408";

/// One file of the model: the name it has in the repository, the name the loader looks for, its size and its SHA-256.
#[derive(Clone, Copy, Debug)]
pub struct ModelFile {
    pub remote: &'static str,
    pub local: &'static str,
    pub size: u64,
    pub sha256: &'static str,
}

pub const FILES: [ModelFile; 4] = [
    ModelFile {
        remote: "encoder-model.exact4x2.onnx",
        local: "encoder-model.onnx",
        size: 662_190_977,
        sha256: "abfdefaa1c74d6d3ca367a7ed358732a6140fb26a312b650ee57e46f1a9849ec",
    },
    ModelFile {
        remote: "decoder_joint-model.exact4x2.onnx",
        local: "decoder_joint-model.onnx",
        size: 72_518_934,
        sha256: "420125e0e13596692320c35ef648eee9bf4583718c7896c8732ebf6f50b9ca0d",
    },
    ModelFile { remote: "vocab.txt", local: "vocab.txt", size: 93_939, sha256: "d58544679ea4bc6ac563d1f545eb7d474bd6cfa467f0a6e2c1dc1c7d37e3c35d" },
    ModelFile { remote: "config.json", local: "config.json", size: 121, sha256: "db59e29a3c1fde6a081bf04965e72bba26cd65be1aee65b064360df8aef468e5" },
];

/// Every byte of the model.
pub fn total_bytes(files: &[ModelFile]) -> u64 {
    files.iter().map(|f| f.size).sum()
}

/// `<data>/atelier/speech/phonon-2`, where the model lives on this machine.
pub fn dir() -> Option<PathBuf> {
    Some(dirs::data_dir()?.join("atelier").join("speech").join("phonon-2"))
}

/// Whether every file is there at its pinned size. They are hashed when they arrive, so the size is enough to ask later.
pub fn installed(dir: &Path, files: &[ModelFile]) -> bool {
    files.iter().all(|f| fs::metadata(dir.join(f.local)).is_ok_and(|m| m.len() == f.size))
}

/// Fetches what is missing into `dir`, from `base`, and tells `progress` how many bytes of the whole are in place. A file is
/// written as `<name>.part`, checked against its hash, and only then renamed, so a stopped download never looks installed.
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
        let mut response = agent.get(&format!("{base}/{}", file.remote)).call().map_err(|why| Error::Network(why.to_string()))?;
        let mut reader = response.body_mut().as_reader();
        let mut out = fs::File::create(&part)?;
        let mut hash = Sha256::new();
        let mut buffer = vec![0u8; 256 * 1024];
        let mut written = 0u64;
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

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests;
