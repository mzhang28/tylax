//! End-to-end Typst→LaTeX tests.
//!
//! The primary fixture is `tests/fixtures/algebra/main.typ` (a real course-notes
//! chapter). It exercises the full pipeline; it is *not* expected to convert
//! every construct (it uses cetz/fletcher diagrams, wordometer, biceps, a
//! bibliography, ...), so it is converted in `Raw` mode and snapshotted. The
//! smaller tests below pin the unsupported-construct policy and the hard-stop
//! behaviour.
//!
//! External-compiler steps use `tectonic` / `typst` when present and are
//! skipped (not failed) otherwise, so `cargo test` stays green in minimal
//! environments. Set `TYLAX_BLESS=1` to (re)write golden files.

use std::path::{Path, PathBuf};
use std::process::Command;

use tempfile::TempDir;
use tylax::core::typst2latex::convert;
use tylax::{T2LOptions, UnsupportedMode};

fn fixtures_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

fn have_binary(name: &str) -> bool {
    Command::new(name)
        .arg("--version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// An auto-cleaned scratch directory. It is created under the crate's
/// `target/` rather than the system temp dir because some tools in this
/// environment fail to write PDFs under `/tmp`, but the repo filesystem is
/// always writable. The returned `TempDir` removes it on drop.
fn scratch_dir() -> TempDir {
    let base = Path::new(env!("CARGO_MANIFEST_DIR")).join("target/tylax-test-tmp");
    std::fs::create_dir_all(&base).unwrap();
    TempDir::new_in(&base).expect("failed to create scratch dir")
}

/// Golden-compare `actual` against `<fixture>/expected/main.tex`.
fn assert_golden(name: &str, actual: &str) {
    let expected_path = fixtures_dir().join(name).join("expected/main.tex");
    if std::env::var("TYLAX_BLESS").is_ok() || !expected_path.exists() {
        std::fs::create_dir_all(expected_path.parent().unwrap()).unwrap();
        std::fs::write(&expected_path, actual).unwrap();
        return;
    }
    let expected = std::fs::read_to_string(&expected_path).unwrap();
    assert_eq!(
        actual.trim_end(),
        expected.trim_end(),
        "generated LaTeX for `{name}` differs from golden {expected_path:?}; \
         re-run with TYLAX_BLESS=1 if the change is intended",
    );
}

/// The primary end-to-end test: the algebra chapter must convert without
/// panicking (in `Raw` mode, so unsupported constructs become visible markers
/// rather than hard errors), and its output is snapshotted for regressions.
#[test]
fn algebra_converts_and_snapshots() {
    let main = fixtures_dir().join("algebra/main.typ");
    let root = fixtures_dir().join("algebra");
    let options = T2LOptions {
        full_document: true,
        unsupported: UnsupportedMode::Raw,
        ..Default::default()
    };
    let tex = convert(&main, &root, &options)
        .expect("algebra should convert in Raw mode")
        .output;

    assert!(tex.contains("\\begin{document}"), "expected a full document wrapper");
    assert!(tex.contains("\\section"), "expected section headings");
    assert_golden("algebra", &tex);

    // Best-effort: if tectonic is available, try to compile. The algebra
    // chapter is not expected to compile cleanly yet (unsupported markers,
    // bibliography, ...), so this is informational, not an assertion.
    if have_binary("tectonic") {
        let dir = scratch_dir();
        let tex_path = dir.path().join("main.tex");
        std::fs::write(&tex_path, &tex).unwrap();
        let status = Command::new("tectonic")
            .arg("--outdir").arg(dir.path())
            .arg(&tex_path)
            .status();
        match status {
            Ok(s) if s.success() => eprintln!("algebra: tectonic compiled cleanly"),
            Ok(_) => eprintln!("algebra: tectonic did not compile cleanly (expected for now)"),
            Err(e) => eprintln!("algebra: tectonic failed to run: {e}"),
        }
    }
}

/// The algebra fixture must itself be valid Typst (sanity-checks the fixture).
#[test]
fn algebra_fixture_is_valid_typst() {
    if !have_binary("typst") {
        eprintln!("skipping typst compile (binary not found)");
        return;
    }
    let main = fixtures_dir().join("algebra/main.typ");
    let dir = scratch_dir();
    let out_pdf = dir.path().join("out.pdf");
    let out = Command::new("typst")
        .arg("compile")
        .arg(&main)
        .arg(&out_pdf)
        .output()
        .expect("failed to run typst");
    assert!(
        out.status.success(),
        "typst failed to compile the algebra fixture:\n{}",
        String::from_utf8_lossy(&out.stderr),
    );
}

/// A construct that evaluates fine but cannot be lowered must be reported under
/// the default `Error` mode and, under `Raw`, emitted as a visible marker
/// rather than silently vanishing.
#[test]
fn unsupported_policy_error_vs_raw() {
    let dir = scratch_dir();
    let main = dir.path().join("main.typ");
    std::fs::write(&main, "#circle(radius: 5pt)\n\nhi\n").unwrap();
    let err_opts = T2LOptions { full_document: true, unsupported: UnsupportedMode::Error, ..Default::default() };
    let raw_opts = T2LOptions { full_document: true, unsupported: UnsupportedMode::Raw, ..Default::default() };

    assert!(convert(&main, dir.path(), &err_opts).is_err(), "circle should be unsupported under Error mode");

    let raw = convert(&main, dir.path(), &raw_opts).expect("raw mode should not error").output;
    assert!(raw.contains("[unsupported Typst element: circle]"), "raw mode should emit a visible marker");
}

/// A Typst evaluation failure (e.g. calling an undefined function) must stop
/// conversion rather than emit plausible-but-wrong LaTeX.
#[test]
fn eval_error_hard_stops() {
    let dir = scratch_dir();
    let main = dir.path().join("main.typ");
    std::fs::write(&main, "#unknown-function(1, 2, 3)\n").unwrap();
    let opts = T2LOptions { full_document: true, ..Default::default() };
    assert!(convert(&main, dir.path(), &opts).is_err(), "eval error must hard-stop");
}
