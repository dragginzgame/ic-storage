//! Consumer selection, evidence and recovery checks with substituted Git/Make effects.
use super::*;
use std::fmt::Write as _;
use validation_plan::{Plan, canonical};
static STAGES: std::sync::LazyLock<Vec<String>> = std::sync::LazyLock::new(|| {
    let raw = fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../ci/release-validation.json"),
    )
    .unwrap();
    serde_json::from_value(
        serde_json::from_str::<serde_json::Value>(&raw).unwrap()["stages"].clone(),
    )
    .unwrap()
});
fn stages(group: &str, changed: &[String], baseline: bool) -> Vec<validation_plan::Stage> {
    validation_plan::stages(group, changed, baseline, &STAGES)
}

// Native fixtures substitute the Shared process boundary; the real runner test
// below invokes the selected verifier over actual retained checkpoints and logs.
pub(super) fn shared_proof(root: &Path, args: &[&str], git: &GitState) -> Result<String> {
    if args.get(1) == Some(&"baseline") {
        let changes: Vec<_> = git
            .validation_baseline
            .as_ref()
            .map_or_else(Vec::new, |base| {
                git.source
                    .iter()
                    .filter(|(path, text)| base.get(*path) != Some(*text))
                    .map(|(path, _)| path.clone())
                    .collect()
            });
        return Ok(String::from_utf8(canonical(&serde_json::json!({
            "schema":1, "source":args[3], "reference":args[4],
            "available":git.validation_baseline.is_some(), "baseline":"baseline", "changes":changes,
        }))?)?);
    }
    require(
        args.len() == 8 && args[1] == "verify",
        "wrong evidence dispatch",
    )?;
    let path = args[3];
    let run = args[6];
    let mut files = BTreeMap::new();
    for entry in fs::read_dir(root.join(run))? {
        let entry = entry?;
        files.insert(
            entry
                .file_name()
                .to_str()
                .ok_or("fixture filename")?
                .to_owned(),
            Sha256Digest::compute(&fs::read(entry.path())?).to_string(),
        );
    }
    Ok(String::from_utf8(canonical(&serde_json::json!({
        "schema":1,"kind":"group-validation-evidence","source":args[4],"group":args[5],"run":run,
        "files":files,"plan_sha256":Sha256Digest::compute(&fs::read(root.join(path))?).to_string(),
        "context_sha256":Sha256Digest::compute(&fs::read(root.join(args[7]))?).to_string()
    }))?)?)
}

// Only the process boundary is substituted: actual receipt admission below reads
// the same plan/checkpoint/event shapes emitted by Shared's separately exercised runner.
pub(super) fn runner_evidence(root: &Path, args: &[&str]) -> Result<()> {
    require(args.len() == 7, "unexpected validation dispatch")?;
    let path = args[5]
        .strip_prefix("VALIDATION_PLAN=")
        .ok_or("missing fixture plan")?;
    let plan: Plan = serde_json::from_slice(&fs::read(root.join(path))?)?;
    require(
        args[3] == format!("RELEASE_SOURCE={}", plan.source)
            && args[4] == format!("RELEASE_GROUP={}", plan.group)
            && args[6] == format!("VALIDATION_LOG_DIR={path}.logs"),
        "wrong plan dispatch",
    )?;
    let run = root.join(format!("{path}.logs/validation.fixture"));
    // Positional nested runners share the retained log root without owning
    // this release plan. They must neither qualify nor invalidate its receipt.
    fs::create_dir_all(root.join(format!("{path}.logs/validation.nested")))?;
    fs::create_dir_all(&run)?;
    fs::copy(root.join(path), run.join("parsed-inputs.json.plan.json"))?;
    fs::copy(
        root.join(&plan.policy),
        run.join("parsed-inputs.json.policy.json"),
    )?;
    let mut inputs = BTreeMap::new();
    for input in plan.inputs.iter().chain(std::iter::once(&path.to_owned())) {
        inputs.insert(
            input.clone(),
            Sha256Digest::compute(&fs::read(root.join(input))?).to_string(),
        );
    }
    fs::write(run.join("plan-inputs.json"), canonical(&inputs)?)?;
    let mut events = "target\tstate\treason\n".to_owned();
    let mut timings = "target\tresult\tseconds\tlog\n".to_owned();
    for stage in &plan.stages {
        writeln!(
            events,
            "{}\t{}\t{}",
            stage.target,
            if stage.required {
                "blocked"
            } else {
                "not-applicable"
            },
            stage.reason
        )?;
    }
    for stage in plan.stages.iter().filter(|stage| stage.required) {
        let log = run.join(format!("{}.log", stage.target));
        fs::write(&log, format!("substituted Make target {}\n", stage.target))?;
        writeln!(events, "{}\tPASS\t{}", stage.target, log.display())?;
        writeln!(timings, "{}\tPASS\t0\t{}", stage.target, log.display())?;
    }
    fs::write(run.join("plan-events.tsv"), events)?;
    fs::write(run.join("timings.tsv"), timings)?;
    Ok(())
}

#[test]
fn admitted_adapter_baselines_survive_recovery_and_refuse_drift() {
    for group in ["metrics-storage", "logging-storage"] {
        let fixture = Fixture::for_group(group);
        let path = format!("crates/ic-{group}/src/tests/mod.rs");
        let file = fixture.repo.root.join(&path);
        fs::create_dir_all(file.parent().unwrap()).unwrap();
        fs::write(file, "// selected adapter change\n").unwrap();
        {
            let mut state = fixture.repo.exec.state.borrow_mut();
            state.git.validation_baseline = Some(state.git.source.clone());
            state
                .git
                .source
                .insert(path, "// selected adapter change\n".into());
        }
        fixture.repo.verify(&fixture.selection).unwrap();
        let receipt = fixture.validation_evidence();
        assert_eq!(receipt.group_validation.plan.scope, "group");
        assert_eq!(
            receipt
                .group_validation
                .plan
                .stages
                .iter()
                .filter(|stage| stage.required)
                .count(),
            8
        );
        fixture.repo.validated(&fixture.selection).unwrap();
        fixture.repo.exec.state.borrow_mut().git.validation_baseline = None;
        assert!(fixture.repo.validated(&fixture.selection).is_err());
    }
    let fixture = Fixture::new();
    {
        let mut state = fixture.repo.exec.state.borrow_mut();
        let mut baseline = state.git.source.clone();
        baseline.insert(PACKAGE_MANIFEST.into(), PACKAGE.replace("0.12.3", "0.12.2"));
        state.git.validation_baseline = Some(baseline);
    }
    assert!(fixture.repo.verify(&fixture.selection).is_err());
    assert!(
        !fixture
            .repo
            .exec
            .state
            .borrow()
            .has_call(|call| is_make_call(call, "validate-release-plan"))
    );
}

#[test]
fn adapter_private_changes_select_only_proven_unaffected_omissions() {
    for (group, path) in [
        ("metrics-storage", "codec/mod.rs"),
        ("logging-storage", "codec/mod.rs"),
    ] {
        let selected = stages(group, &[format!("crates/ic-{group}/src/{path}")], true);
        assert_eq!(selected.len(), STAGES.len());
        assert!(
            !selected
                .iter()
                .find(|s| s.target == "validate-tooling")
                .unwrap()
                .required
        );
        assert!(selected.iter().find(|s| s.target == format!("validate-{}-tests", group.split('-').next().unwrap())).unwrap().required);
        for target in [
            "validate-lint",
            "validate-features",
            "validate-policy",
            "validate-wasm",
            "validate-archives",
        ] {
            assert!(
                selected
                    .iter()
                    .find(|s| s.target == target)
                    .unwrap()
                    .required
            );
        }
        for shared in [
            "Cargo.lock",
            "Cargo.toml",
            "rust-toolchain.toml",
            "Makefile",
            "ci/tool-versions.env",
            ".github/workflows/ci.yml",
            "unknown",
            "crates/ic-storage/src/lib.rs",
        ] {
            assert!(
                stages(group, &[shared.to_owned()], true)
                    .iter()
                    .all(|s| s.required)
            );
        }
        assert!(
            stages(group, &[format!("crates/ic-{group}/src/lib.rs")], true)
                .iter()
                .all(|s| s.required)
        );
        assert!(
            stages(group, &[format!("crates/ic-{group}/src/{path}")], false)
                .iter()
                .all(|s| s.required)
        );
    }
    assert!(
        stages("storage", &["crates/ic-storage/src/lib.rs".into()], true)
            .iter()
            .all(|s| s.required)
    );
}

#[test]
fn required_failures_and_incomplete_evidence_never_publish_validation() {
    let fixture = Fixture::new();
    fixture.fail_build(BuildFault::Validate);
    assert!(fixture.repo.verify(&fixture.selection).is_err());
    assert!(
        !fixture
            .repo
            .validation_path(&fixture.selection.version)
            .unwrap()
            .exists()
    );
    fixture.repo.exec.state.borrow_mut().clear_faults();
    fixture.repo.verify(&fixture.selection).unwrap();
    let receipt = fixture.validation_evidence();
    assert_eq!(receipt.schema, 2);
    let events = fixture
        .repo
        .root
        .join(&receipt.group_validation.runner)
        .join("plan-events.tsv");
    let original = fs::read_to_string(&events).unwrap();
    for corrupted in [
        original.replace("\tPASS\t", "\tFAIL\t"),
        original
            .lines()
            .take(STAGES.len() + 1)
            .collect::<Vec<_>>()
            .join("\n"),
        format!("{original}validate-lint\tPASS\twrong.log\n"),
    ] {
        fs::write(&events, corrupted).unwrap();
        assert!(fixture.repo.validated(&fixture.selection).is_err());
    }
    fs::write(&events, original).unwrap();
    fixture.repo.validated(&fixture.selection).unwrap();
    let log = fixture
        .repo
        .root
        .join(&receipt.group_validation.runner)
        .join("validate-lint.log");
    fs::write(&log, "changed proof").unwrap();
    assert!(fixture.repo.validated(&fixture.selection).is_err());
    fixture.repo.verify(&fixture.selection).unwrap();
    fixture.repo.validated(&fixture.selection).unwrap();
}

#[test]
fn altered_policy_roster_and_legacy_receipts_refuse() {
    let fixture = Fixture::new();
    fixture.repo.verify(&fixture.selection).unwrap();
    let path = fixture
        .repo
        .validation_path(&fixture.selection.version)
        .unwrap();
    let mut receipt: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    receipt["schema"] = serde_json::json!(1);
    fs::write(&path, serde_json::to_vec(&receipt).unwrap()).unwrap();
    assert!(fixture.repo.validated(&fixture.selection).is_err());
    fixture.repo.verify(&fixture.selection).unwrap();
    let policy = fixture
        .repo
        .text(validation_plan::POLICY, Some("source"))
        .unwrap();
    fixture.set_source(
        validation_plan::POLICY,
        &policy.replace("validate-lint", "unexpected-stage"),
    );
    assert!(fixture.repo.verify(&fixture.selection).is_err());
}

#[cfg(unix)]
#[test]
fn shared_runner_matches_full_roster_and_honors_consumer_exclusions() {
    use std::os::unix::fs::PermissionsExt as _;
    let fixture = Fixture::new();
    let root = &fixture.repo.root;
    let bin = root.join("bin");
    fs::create_dir(&bin).unwrap();
    let write_script = |name: &str, text: &str| {
        let path = bin.join(name);
        fs::write(&path, text).unwrap();
        fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
    };
    write_script("git", "#!/bin/sh\ncat \"$FIXTURE_ROOT/head\"\n");
    write_script(
        "make",
        r#"#!/usr/bin/env bash
set -euo pipefail
for target in "$@"; do
    case "$target" in
        validate-*)
            printf '%s\n' "$target" >> "$FIXTURE_ROOT/calls"
            [[ "$target" != "$FAIL_STAGE" ]] || exit 17
            if [[ "$target" == "$DRIFT_INPUT" ]]; then printf '# changed input\n' >> "$FIXTURE_ROOT/Makefile"; fi
            if [[ "$target" == "$DRIFT_STAGE" ]]; then printf 'bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb\n' > "$FIXTURE_ROOT/head"; fi
            exit 0 ;;
    esac
done
exec /usr/bin/make "$@"
"#,
    );
    let source = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    let checkout = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    for helper in [
        "run-validation-targets.sh",
        "check-make-execution.sh",
        "verify-file-checksum.sh",
        "validation-input-checkpoint.pl",
        "release-validation-plan.pl",
        "ValidationPlan.pm",
        "validation-evidence.pl",
    ] {
        let path = format!("scripts/ci/{helper}");
        fixture.set_source(&path, &fs::read_to_string(checkout.join(&path)).unwrap());
    }
    let path_env = format!("{}:{}", bin.display(), env::var("PATH").unwrap());
    let mut iteration = 0;
    for (group, private) in [
        ("storage", "src/lib.rs"),
        ("metrics-storage", "src/codec/mod.rs"),
        ("logging-storage", "src/codec/mod.rs"),
    ] {
        for scenario in [
            "workspace",
            "group",
            "required-failure",
            "excluded-failure",
            "drift",
            "inputs",
        ] {
            iteration += 1;
            let path = format!("target/runner-{iteration}/plan.json");
            let directory = root.join(&path).parent().unwrap().to_owned();
            fs::create_dir_all(&directory).unwrap();
            let (mut plan, mut context) = fixture
                .repo
                .validation_selection(&fixture.selection, &path)
                .unwrap();
            plan.source = source.to_owned();
            plan.group = group.to_owned();
            context.source = source.to_owned();
            context.group = group.to_owned();
            fs::write(
                root.join(format!("{path}.context.json")),
                canonical(&context).unwrap(),
            )
            .unwrap();
            plan.stages = stages(
                group,
                &[format!("crates/ic-{group}/{private}")],
                scenario != "workspace",
            );
            // Exercise UTF-8 through the actual runner, shared verifier and
            // Storage receipt admission, rather than only parsing a mock proof.
            for stage in plan.stages.iter_mut().filter(|stage| !stage.required) {
                stage.reason.push_str(" — other group’s checks");
            }
            plan.scope = if plan.stages.iter().all(|s| s.required) {
                "workspace"
            } else {
                "group"
            }
            .into();
            fs::write(root.join(&path), canonical(&plan).unwrap()).unwrap();
            fs::write(root.join("head"), format!("{source}\n")).unwrap();
            fs::write(root.join("calls"), "").unwrap();
            fs::write(
                root.join("Makefile"),
                fixture.repo.text("Makefile", Some("source")).unwrap(),
            )
            .unwrap();
            let output = Command::new("bash")
                .arg("scripts/ci/run-validation-targets.sh")
                .args(["--plan", &path])
                .current_dir(root)
                .env("PATH", &path_env)
                .env("FIXTURE_ROOT", root)
                .env("RELEASE_SOURCE", source)
                .env("RELEASE_GROUP", group)
                .env("VALIDATION_LOG_DIR", format!("{path}.logs"))
                .env("TMPDIR", root)
                .env_remove("VALIDATION_INPUT_ROOT")
                .env_remove("VALIDATION_INPUT_BASELINE")
                .env(
                    "FAIL_STAGE",
                    match scenario {
                        "required-failure" => "validate-lint",
                        "excluded-failure" => "validate-tooling",
                        _ => "",
                    },
                )
                .env(
                    "DRIFT_INPUT",
                    if scenario == "inputs" {
                        "validate-lint"
                    } else {
                        ""
                    },
                )
                .env(
                    "DRIFT_STAGE",
                    if scenario == "drift" {
                        "validate-lint"
                    } else {
                        ""
                    },
                )
                .output()
                .unwrap();
            fs::write(directory.join("stdout.log"), &output.stdout).unwrap();
            fs::write(directory.join("stderr.log"), &output.stderr).unwrap();
            let success = !(matches!(scenario, "required-failure" | "drift" | "inputs")
                || scenario == "excluded-failure" && group == "storage");
            assert_eq!(
                output.status.success(),
                success,
                "{group}/{scenario}: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            let calls = fs::read_to_string(root.join("calls")).unwrap();
            if success {
                assert_eq!(
                    calls.lines().collect::<Vec<_>>(),
                    plan.stages
                        .iter()
                        .filter(|s| s.required)
                        .map(|s| s.target.as_str())
                        .collect::<Vec<_>>()
                );
            }
            let repository = Repository {
                root: root.clone(),
                exec: Processes,
                group: group.to_owned(),
            };
            let mut selection = fixture.selection.clone();
            selection.source = source.to_owned();
            selection.group = group.to_owned();
            let collected = repository.collect_validation(&selection, &path, plan, context);
            if success {
                let mut record = collected.unwrap();
                repository
                    .check_validation_records(&selection, &mut record, false)
                    .unwrap();
                let log = root.join(&record.runner).join("0.log");
                let original = fs::read(&log).unwrap();
                fs::write(&log, b"changed consumer evidence").unwrap();
                assert!(
                    repository
                        .check_validation_records(&selection, &mut record, false)
                        .is_err()
                );
                fs::write(&log, original).unwrap();
                repository
                    .check_validation_records(&selection, &mut record, false)
                    .unwrap();
                let proof_path = root.join(format!("{path}.proof.json"));
                let proof = fs::read(&proof_path).unwrap();
                fs::remove_file(&proof_path).unwrap();
                assert!(
                    repository
                        .check_validation_records(&selection, &mut record, false)
                        .is_err()
                );
                fs::write(&proof_path, proof).unwrap();
                repository
                    .check_validation_records(&selection, &mut record, false)
                    .unwrap();
            } else {
                assert!(collected.is_err(), "{group}/{scenario} cannot qualify");
                assert!(!root.join(format!("{path}.proof.json")).exists());
            }
            if !success && matches!(scenario, "required-failure" | "drift" | "inputs") {
                assert_eq!(
                    calls.lines().collect::<Vec<_>>(),
                    ["validate-admission", "validate-lint"]
                );
            }
        }
    }
}

#[test]
fn reverse_dependencies_disable_adapter_omissions_even_after_a_new_baseline() {
    assert!(validation_plan::independent_adapter(&fixture_lock(), "metrics-storage").unwrap());
    let lock = fixture_lock().replace(
        "name = \"ic-storage\"\nversion = \"0.12.3\"",
        "name = \"ic-storage\"\nversion = \"0.12.3\"\ndependencies = [\"bridge\"]",
    ) + "\n[[package]]\nname = \"bridge\"\nversion = \"1.0.0\"\ndependencies = [\"ic-metrics-storage 0.12.3\"]\n";
    assert!(!validation_plan::independent_adapter(&lock, "metrics-storage").unwrap());
}
