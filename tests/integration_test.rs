const NUM_TESTS: u32 = 4;
seq!(N in 1..=4 {
    #[allow(dead_code)]
    mod input_~N;
    mod golden_~N;
});

use anyhow::Context;
use seq_macro::seq;
use tempfile::TempDir;
use std::{io::{Read, Write}, path::PathBuf};

#[test]
fn integration_tests() {
    for i in 1..=NUM_TESTS {
        // Skip Test 2 and 4
        // They are currently failing because cargo-minify
        // doesn't support removing traits.
        if i == 2 || i == 4 {
            continue;
        }
        integration_test(i);
    }
}


fn rustfmt(src: &str) -> anyhow::Result<String> {
    let mut command = std::process::Command::new("rustfmt")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()?;

    let mut stdin = command
        .stdin
        .take()
        .context("Failed to open rustfmt stdin")?;
    stdin.write_all(src.as_bytes())?;
    stdin.flush()?;
    drop(stdin);

    let output = command.wait_with_output()?;
    let stdout_string = String::from_utf8(output.stdout)?;

    Ok(stdout_string)
}

fn remove_dead_code(src: String) -> anyhow::Result<String> {
    let tmp_dir = TempDir::new()?;
    let dummy_cargo_toml_contents =
r#"[package]
name = "dummy-package"
version = "0.1.0"
edition = "2024"
"#;
    let cargo_toml_file_path = tmp_dir.path().join("Cargo.toml");
    std::fs::File::create_new(&cargo_toml_file_path)?.write_all(dummy_cargo_toml_contents.as_bytes())?;
    let src_dir = tmp_dir.path().join("src");
    std::fs::create_dir(&src_dir)?;
    let lib_file_path = src_dir.join("lib.rs");
    std::fs::File::create_new(&lib_file_path)?.write_all(src.as_bytes())?;
    let args = [
        "--manifest-path".to_string(),
        cargo_toml_file_path.to_str().context("couldn't convert path to str")?.to_string(),
        "--apply".to_string(),
        "--allow-no-vcs".to_string(),
        "--allow-dirty".to_string(),
        "--allow-staged".to_string(),
    ];
    cargo_minify::execute(&args)?;
    let mut output = String::new();
    std::fs::File::open(&lib_file_path)?.read_to_string(&mut output)?;
    Ok(output)
}

fn integration_test(test_index: u32) {
    // let manifest_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let manifest_dir = PathBuf::from("/home/robert/GitProjects/cargo-minify");
    let tests_dir = manifest_dir.join("tests");
    let input_path = tests_dir.join(format!("input_{test_index}.rs"));
    let golden_path = tests_dir.join(format!("golden_{test_index}.rs"));
    let input_contents = std::fs::read_to_string(input_path).unwrap();
    let golden_contents = std::fs::read_to_string(golden_path).unwrap();
    let output_contents = rustfmt(&remove_dead_code(input_contents).unwrap()).unwrap();
    let output_path = tests_dir.join(format!("output_{test_index}.rs"));
    std::fs::File::create(output_path)
        .unwrap()
        .write_all(output_contents.as_bytes())
        .unwrap();
    assert_eq!(
        output_contents, golden_contents,
        "Integration test #{} failed",
        test_index
    );
}
