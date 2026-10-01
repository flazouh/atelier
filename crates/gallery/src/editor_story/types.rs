use super::structs::Fixture;

pub(super) const FIXTURES: &[Fixture] = &[
    Fixture {
        label: "Rust",
        project: &[("Cargo.toml", "[package]\nname = \"atelier-gallery\"\nversion = \"0.1.0\"\nedition = \"2021\"\n")],
        file: "src/lib.rs",
        text: SAMPLE_RUST,
    },
    Fixture {
        label: "TypeScript",
        project: &[("tsconfig.json", "{ \"compilerOptions\": { \"strict\": true } }\n")],
        file: "src/shapes.ts",
        text: "// `width` is used twice below: Cmd-click its declaration to list both.\n\nexport function width(): number {\n  return 7;\n}\n\nexport function area(): number {\n  return width() * width();\n}\n\nexport function broken(): number {\n  const text: number = \"not a number\";\n  return text + width();\n}\n",
    },
    Fixture {
        label: "Python",
        project: &[("pyproject.toml", "[project]\nname = \"fixture\"\nversion = \"0.1.0\"\n")],
        file: "shapes.py",
        text: "# `width` is used twice below: Cmd-click its declaration to list both.\n\n\ndef width() -> int:\n    return 7\n\n\ndef area() -> int:\n    return width() * width()\n\n\ndef broken() -> int:\n    text: int = \"not a number\"\n    return text + width()\n",
    },
    Fixture {
        label: "Go",
        project: &[("go.mod", "module example.com/shapes\n\ngo 1.21\n")],
        file: "shapes.go",
        text: "// Package shapes: `width` is used twice below; Cmd-click its declaration to list both.\npackage shapes\n\nfunc width() int {\n\treturn 7\n}\n\nfunc area() int {\n\treturn width() * width()\n}\n\nfunc broken() int {\n\tvar text int = \"not a number\"\n\treturn text + width()\n}\n",
    },
    Fixture {
        label: "Java",
        project: &[("pom.xml", "<project><modelVersion>4.0.0</modelVersion><groupId>shapes</groupId><artifactId>shapes</artifactId><version>1</version></project>\n")],
        file: "src/main/java/Shapes.java",
        text: "// `width` is used twice below: Cmd-click its declaration to list both.\npublic class Shapes {\n    static int width() {\n        return 7;\n    }\n\n    static int area() {\n        return width() * width();\n    }\n\n    static int broken() {\n        int text = \"not a number\";\n        return text + width();\n    }\n}\n",
    },
];

/// The Rust tab's file: the one the editor was first built on.
pub(super) const SAMPLE_RUST: &str = include_str!("../editor_story/sample_lib.rs.txt");
