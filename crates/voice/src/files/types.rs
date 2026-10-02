use super::structs::ModelFile;

/// Where the files are fetched from, pinned to a commit.
pub const BASE: &str = "https://huggingface.co/istupakov/parakeet-tdt-0.6b-v2-onnx/resolve/0bbb45a3365852604aef28b538a8f066f4ccaa85";

pub const FILES: [ModelFile; 4] = [
    ModelFile {
        remote: "encoder-model.int8.onnx",
        local: "encoder-model.int8.onnx",
        size: 652_184_014,
        sha256: "3e0581fda6ab843888b51e56d7ee78b6d5bc3237ec113af1f732d1d5286aa155",
    },
    ModelFile {
        remote: "decoder_joint-model.int8.onnx",
        local: "decoder_joint-model.int8.onnx",
        size: 8_998_286,
        sha256: "a449f49acd68979d418651dd2dcb737cc0f1bf0225e009e29ee326354edbf7d3",
    },
    ModelFile { remote: "vocab.txt", local: "vocab.txt", size: 9_384, sha256: "ec182b70dd42113aff6c5372c75cac58c952443eb22322f57bbd7f53977d497d" },
    ModelFile { remote: "config.json", local: "config.json", size: 97, sha256: "666903c76b9798caf2c210afd4f6cd60b08a8dbf9800ec8d7a3bc0d2148ac466" },
];
