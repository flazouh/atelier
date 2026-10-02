use super::structs::ModelFile;

/// Where the files are fetched from, pinned to a commit.
pub const BASE: &str = "https://huggingface.co/tiyuvta/Phonon-2-ONNX/resolve/12c9688bbc4fc52d23c1a66ca873fd3ac6ed4408";

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
