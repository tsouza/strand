// Copyright the STRAND authors.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! Runs the actual compiled `strand-tools` binary as a subprocess — the
//! level at which the CLI's argument parsing and exit codes are real, not
//! just the pure formatting logic `inspect.rs`'s own unit tests cover.

use std::process::Command;

fn strand_tools() -> Command {
    Command::new(env!("CARGO_BIN_EXE_strand-tools"))
}

#[test]
fn inspect_prints_a_report_and_exits_zero() {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../conformance/container/toy-segment.bin"
    );

    let output = strand_tools().args(["inspect", path]).output().unwrap();

    assert!(output.status.success(), "{output:?}");
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("file size: 102 bytes"), "{stdout}");
    assert!(
        stdout.contains("row-ids: [1000, 1002) (count=2)"),
        "{stdout}"
    );
}

#[test]
fn inspect_of_a_missing_file_exits_nonzero_with_a_clear_error() {
    let output = strand_tools()
        .args(["inspect", "/nonexistent/path/does-not-exist.bin"])
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("does-not-exist.bin"), "{stderr}");
}

#[test]
fn inspect_of_a_corrupt_file_exits_nonzero_with_a_clear_error() {
    let dir = std::env::temp_dir();
    let path = dir.join("strand-tools-inspect-corrupt-test.bin");
    std::fs::write(&path, [0u8; 40]).unwrap();

    let output = strand_tools()
        .args(["inspect", path.to_str().unwrap()])
        .output()
        .unwrap();

    std::fs::remove_file(&path).ok();

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("invalid footer"), "{stderr}");
}
