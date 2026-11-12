//! CI event admission and dispatch, without fixture Git or release effects.
use super::super::ci_validation::{changed_paths, selected_group};
use super::*;
use std::fmt::Write as _;

const HEAD: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const BASE: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

fn raw(paths: &[&str]) -> String {
    paths.iter().fold(String::new(), |mut output, path| {
        write!(output, ":100644 100644 {BASE} {HEAD} M\0{path}\0").unwrap();
        output
    })
}

struct CiEffects<'a> {
    inner: &'a Substitute,
    diff: String,
    missing: bool,
    fail: &'a str,
    calls: RefCell<Vec<String>>,
}
impl Execute for CiEffects<'_> {
    fn capture(&self, root: &Path, program: &str, args: &[&str]) -> Result<String> {
        if program == "git" {
            match args {
                ["rev-parse", "HEAD"] => return Ok(HEAD.into()),
                ["cat-file", "-e", _] => {
                    return if self.missing {
                        Err("missing history".into())
                    } else {
                        Ok(String::new())
                    };
                }
                ["merge-base", BASE, HEAD] => return Ok(BASE.into()),
                [
                    "diff",
                    "--raw",
                    "-z",
                    "--no-renames",
                    "--no-abbrev",
                    "--ignore-submodules=none",
                    "--no-relative",
                    BASE,
                    HEAD,
                    "--",
                ] => return Ok(self.diff.clone()),
                ["show", path] if path.starts_with(HEAD) => {
                    return self.inner.capture(
                        root,
                        program,
                        &["show", &path.replacen(HEAD, "source", 1)],
                    );
                }
                ["ls-tree", "-r", "--name-only", "-z", HEAD] => {
                    return self.inner.capture(
                        root,
                        program,
                        &["ls-tree", "-r", "--name-only", "-z", "source"],
                    );
                }
                _ => {}
            }
        }
        self.inner.capture(root, program, args)
    }
    fn execute(&self, root: &Path, program: &str, args: &[&str]) -> Result<()> {
        require(
            program == "make" && args.first() == Some(&"--no-print-directory"),
            "unexpected CI effect",
        )?;
        self.calls.borrow_mut().push(args[1].into());
        if args[1] == "validate-release-plan" {
            let path = args[5]
                .strip_prefix("VALIDATION_PLAN=")
                .ok_or("missing plan")?;
            let plan: validation_plan::Plan = serde_json::from_slice(&fs::read(root.join(path))?)?;
            require(
                plan.scope == "group" && plan.source == HEAD,
                "wrong CI source/scope",
            )?;
            require(
                !plan
                    .stages
                    .iter()
                    .any(|s| s.required && s.target == self.fail),
                "required stage failed",
            )?;
        } else {
            require(args[1] != self.fail, "CI obligation failed")?;
        }
        Ok(())
    }
    fn cargo_target(&self, root: &Path) -> Result<PathBuf> {
        self.inner.cargo_target(root)
    }
}

#[test]
fn ci_diff_and_ownership_refuse_uncertain_scope() {
    let lock = fixture_lock();
    let metrics = "crates/ic-metrics-storage/src/codec/mod.rs";
    let logging = "crates/ic-logging-storage/src/codec/mod.rs";
    assert_eq!(
        selected_group(&changed_paths(&raw(&[metrics])).unwrap(), &lock).unwrap(),
        Some("metrics-storage")
    );
    assert_eq!(
        selected_group(&changed_paths(&raw(&[logging])).unwrap(), &lock).unwrap(),
        Some("logging-storage")
    );
    for paths in [
        vec![metrics, logging],
        vec![metrics, "Cargo.lock"],
        vec![metrics, "Makefile"],
        vec!["crates/ic-storage/src/adapter/tree.rs"],
        vec!["crates/ic-metrics-storage/src/lib.rs"],
        vec!["docs/status/current.md"],
        vec![],
    ] {
        assert_eq!(
            selected_group(
                &paths.into_iter().map(str::to_owned).collect::<Vec<_>>(),
                &lock
            )
            .unwrap(),
            None
        );
    }
    let diff = raw(&[metrics]);
    for invalid in [
        diff.replace("100644 100644", "100644 100755"),
        diff.replace("100644 100644", "100644 120000"),
        diff.replace(" M\0", " R100\0"),
        raw(&["../escape"]),
        diff.trim_end_matches('\0').to_owned(),
        String::new(),
    ] {
        assert!(changed_paths(&invalid).is_err());
    }
    let additional_consumer = format!(
        "{lock}\n[[package]]\nname = \"future-local-consumer\"\nversion = \"1.0.0\"\ndependencies = [\"ic-metrics-storage\"]\n"
    );
    assert_eq!(
        selected_group(&[metrics.into()], &additional_consumer).unwrap(),
        None
    );
    let reverse = lock.replace(
        "name = \"ic-storage\"\nversion = \"0.12.3\"",
        "name = \"ic-storage\"\nversion = \"0.12.3\"\ndependencies = [\"ic-metrics-storage\"]",
    );
    assert_eq!(selected_group(&[metrics.into()], &reverse).unwrap(), None);
}

#[test]
fn ci_dispatch_preserves_full_pushes_fallbacks_and_required_failures() {
    for (event, path, missing, fail, narrow, passes) in [
        (
            "pull_request",
            "crates/ic-metrics-storage/src/codec/mod.rs",
            false,
            "",
            true,
            true,
        ),
        (
            "pull_request",
            "crates/ic-logging-storage/src/codec/mod.rs",
            false,
            "validate-storage-tests",
            true,
            true,
        ),
        (
            "pull_request",
            "crates/ic-logging-storage/src/codec/mod.rs",
            false,
            "validate-lint",
            true,
            false,
        ),
        (
            "pull_request",
            "crates/ic-logging-storage/src/codec/mod.rs",
            false,
            "test-failure-evidence",
            true,
            false,
        ),
        (
            "push",
            "crates/ic-metrics-storage/src/codec/mod.rs",
            false,
            "",
            false,
            true,
        ),
        ("pull_request", "Cargo.lock", false, "", false, true),
        (
            "pull_request",
            "crates/ic-metrics-storage/src/codec/mod.rs",
            true,
            "",
            false,
            true,
        ),
        (
            "pull_request",
            "Makefile",
            false,
            "validate-toolchain",
            false,
            false,
        ),
    ] {
        let fixture = Fixture::new();
        let repo = Repository {
            root: fixture.repo.root.clone(),
            group: String::new(),
            exec: CiEffects {
                inner: &fixture.repo.exec,
                diff: raw(&[path]),
                missing,
                fail,
                calls: RefCell::default(),
            },
        };
        assert_eq!(
            repo.validate_ci(event, BASE, HEAD, "refs/heads/main")
                .is_ok(),
            passes,
            "{event} {path} {fail}"
        );
        let calls = repo.exec.calls.borrow();
        assert_eq!(
            calls[0],
            if narrow {
                "validate-release-plan"
            } else {
                "validate-toolchain"
            }
        );
        assert_eq!(
            calls.iter().any(|c| c == "test-failure-evidence"),
            narrow && fail != "validate-lint"
        );
    }
}

#[test]
fn ci_wrong_head_refuses_before_dispatch() {
    let fixture = Fixture::new();
    let repo = Repository {
        root: fixture.repo.root.clone(),
        group: String::new(),
        exec: CiEffects {
            inner: &fixture.repo.exec,
            diff: raw(&["crates/ic-logging-storage/src/codec/mod.rs"]),
            missing: false,
            fail: "",
            calls: RefCell::default(),
        },
    };
    assert!(
        repo.validate_ci("pull_request", BASE, BASE, "refs/heads/main")
            .is_err()
    );
    assert!(repo.exec.calls.borrow().is_empty());
}
