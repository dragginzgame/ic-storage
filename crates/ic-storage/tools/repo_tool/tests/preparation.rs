//! Preparation release contracts.

use super::*;

#[test]
fn helper_output_cannot_change_locked_dependency_selection() {
    let fixture = Fixture::new();
    let same_name_registry = format!(
        "{}[[package]]\nname = \"ic-storage\"\nversion = \"0.12.3\"\nsource = \"registry+https://github.com/rust-lang/crates.io-index\"\nchecksum = \"unchanged\"\n",
        fixture_lock()
    );
    let rendered = fixture
        .repo
        .replace_lock_version(&same_name_registry, "0.12.3", "0.13.0")
        .unwrap();
    assert!(rendered.contains("name = \"ic-storage\"\nversion = \"0.13.0\""));
    assert!(rendered.contains("name = \"ic-storage\"\nversion = \"0.13.0\""));
    assert!(rendered.contains("name = \"ic-metrics-storage\"\nversion = \"0.12.3\""));
    let changed = fixture_lock().replace("0.12.3", "0.13.0");
    let error = fixture
        .repo
        .check_lock_update(fixture_lock().as_bytes(), &changed, "0.12.3", "0.13.0")
        .unwrap_err();
    assert!(matches!(
        error.downcast_ref::<ic_build::cargo::lock_projection::LockProjectionError>(),
        Some(ic_build::cargo::lock_projection::LockProjectionError::ChangedInputs)
    ));
    fixture.assert_original();
}

#[test]
fn core_projection_and_receipts_preserve_both_adapter_identities() {
    let fixture = Fixture::new();
    fixture.prepare().unwrap();
    let lock: toml::Value =
        toml::from_str(&fixture.repo.text("Cargo.lock", None).unwrap()).unwrap();
    for name in packages() {
        let record = lock["package"]
            .as_array()
            .unwrap()
            .iter()
            .find(|record| record["name"].as_str() == Some(name))
            .unwrap();
        assert_eq!(
            record["version"].as_str(),
            Some(if ["ic-storage"].contains(&name) {
                "0.13.0"
            } else {
                "0.12.3"
            })
        );
    }
    let prepared: PackageEvidence = fixture.package_evidence(Prepared);
    assert_eq!(
        prepared
            .archives()
            .map(|(name, _)| name)
            .collect::<Vec<_>>(),
        ["ic-storage"]
    );
}

#[test]
fn changed_transform_input_is_retained_and_refused_on_retry() {
    let fixture = Fixture::new();
    let repo = Repository {
        group: "storage".to_owned(),
        root: fixture.repo.root.clone(),
        exec: Processes,
    };
    assert!(
        repo.transform_text(
            "sh",
            &["-c", "printf other > \"$1\"; printf ready", "fixture"],
            "input",
            &[],
        )
        .is_err()
    );
    let input = repo
        .target()
        .unwrap()
        .join("release-validation/inputs")
        .join(format!("{}.input", Sha256Digest::compute(b"input")));
    assert_eq!(fs::read(&input).unwrap(), b"other");
    assert!(repo.transform_text("cat", &[], "input", &[]).is_err());
    assert_eq!(fs::read(&input).unwrap(), b"other");
    fixture.assert_original();
}

#[test]
fn large_changelog_finalization_preserves_complete_history() {
    let fixture = Fixture::new();
    let text = format!(
        "## [0.13.0]\n\n{}\n## [0.12.3] - 2026-10-04\n\n## [Draft]\n\nArchived wording.\n",
        "A detailed note.\n".repeat(8192)
    );
    let updated = fixture
        .repo
        .release_changelog(
            &fixture.repo.root.join(CHANGELOG_FINALIZER),
            &text,
            "0.12.3",
            "0.13.0",
            "2026-10-05",
        )
        .unwrap();
    assert_eq!(
        updated,
        text.replacen("## [0.13.0]", "## [0.13.0] - 2026-10-05", 1)
    );
    fixture.assert_original();
}

#[test]
fn release_tool_setup_and_offline_admission_precede_gates_and_version_mutation() {
    for phase in [BuildFault::ToolSetup, BuildFault::ToolCheck] {
        let fixture = Fixture::new();
        fixture.fail_build(phase.clone());
        assert!(fixture.prepare().is_err());
        fixture.assert_original();
        assert!(!fixture.repo.exec.state.borrow().has_call(|call| {
            call.iter()
                .any(|argument| argument == "validate-release-plan" || argument == "fmt-check")
        }));
    }
    let fixture = Fixture::new();
    fixture.prepare().unwrap();
    let state = fixture.repo.exec.state.borrow();
    let position =
        |argument: &str| state.call_position(|call| call.iter().any(|value| value == argument));
    assert!(position("fetch") < position("install-tools"));
    assert!(position("install-tools") < position("tools-check"));
    assert!(position("tools-check") < position("validate-release-plan"));
    let admission = state
        .calls
        .iter()
        .rposition(|call| is_cargo_call(call, "fetch"))
        .unwrap();
    assert!(position("validate-release-plan") < admission);
    assert!(admission < position("fmt-check"));
    assert!(!state.has_call(|call| is_cargo_call(call, "update")));
}

#[test]
fn release_preserves_saved_readme_bytes_without_interpreting_examples() {
    for readme in [
        "ic-storage = \"0.34\"\n",
        "# café\r\n  ic-storage='0.1' # retain \"comment\"\r\n",
        "ic-storage = { features = [\"example\"], version = '0.13', default-features = false }\n",
        "ic-storage = \"0.13.0\"",
        "# Guide without a dependency example\n",
        "ic-storage = \"0.12\"\nic-storage = \"0.13\"\n",
        "ic-storage = \"unterminated\n",
        "ic-storage = { path = \"../ic-storage\" }\n",
    ] {
        let fixture = Fixture::new();
        fixture.set_source(PACKAGE_README, readme);
        // Preserving source bytes must not depend on replacing the same file.
        fixture.fail_files(FileFault::WriteBefore(PACKAGE_README.into()));
        fixture.release();
        fixture.repo.publish(true).unwrap();
        assert_eq!(
            fs::read(fixture.repo.root.join(PACKAGE_README)).unwrap(),
            readme.as_bytes()
        );
    }
}

#[test]
fn release_input_admission_failures_preserve_files_and_receipts() {
    for script in [
        None,
        Some("exit 19;"),
        Some("print \"0\\t13\\n\";"),
        Some("print \"0\\t13\\t0\\textra\\n\";"),
    ] {
        let fixture = Fixture::new();
        let helper = fixture.repo.root.join(RELEASE_INPUT_ADMISSION);
        match script {
            Some(script) => fs::write(helper, script).unwrap(),
            None => fs::remove_file(helper).unwrap(),
        }
        let receipt = fixture.repo.root.join("previous.receipt");
        fs::write(&receipt, b"prior evidence").unwrap();
        assert!(fixture.prepare().is_err());
        fixture.assert_original();
        assert_eq!(fs::read(&receipt).unwrap(), b"prior evidence");
        assert!(!fixture.repo.exec.state.borrow().has_call(|call| {
            matches!(
                call[0].as_str(),
                "make" | "cargo" | "shared-runner" | "write" | "stream-write"
            )
        }));
    }
    let fixture = Fixture::new();
    fs::write(
        fixture.repo.root.join(RELEASE_INPUT_ADMISSION),
        "print \"2000-01-01\\n\";",
    )
    .unwrap();
    assert!(fixture.repo.validate_date("2026-10-15").is_err());
}

#[test]
fn release_versions_and_pending_notes_preserve_dependency_versions_and_history() {
    let fixture = Fixture::new();
    let finalize = |text: &str, version: &str| {
        fixture.repo.release_changelog(
            &fixture.repo.root.join(CHANGELOG_FINALIZER),
            text,
            "0.12.3",
            version,
            "2026-10-05",
        )
    };
    for invalid in [
        "0.01.3",
        "0.12",
        "0.12.3-dev",
        "0.12.+3",
        "18446744073709551616.0.0",
    ] {
        assert!(fixture.repo.version_parts(invalid).is_err());
    }
    assert_eq!(
        fixture
            .repo
            .version_parts("18446744073709551615.0.0")
            .unwrap(),
        [u64::MAX, 0, 0]
    );
    let updated = fixture
        .repo
        .project_catalog(&fixture_manifest(), "0.12.3", "0.13.0")
        .unwrap();
    assert!(updated.contains("other = \"0.12.3\""));
    let current = "# Changelog\n\n## [0.13.0]\n\nChanges.\n\n## 0.12.3\n\nHistory.\n";
    assert_eq!(
        finalize(current, "0.13.0").unwrap(),
        current.replacen("## [0.13.0]", "## [0.13.0] - 2026-10-05", 1)
    );
    assert!(finalize(current, "0.12.4").is_err());
    assert_eq!(
        finalize("## [0.13.0]\n\n## 0.12.3\nHistory", "0.13.0").unwrap(),
        "## [0.13.0] - 2026-10-05\n\n## 0.12.3\nHistory"
    );
    assert_eq!(
        finalize("## [0.13.0]", "0.13.0").unwrap(),
        "## [0.13.0] - 2026-10-05\n"
    );
    for historical in ["0.13.0", "[0.13.0]", "[0.13.0] - 2026-10-05"] {
        let duplicate = format!("{current}\n## {historical}\n\nAlready released.\n");
        assert!(finalize(&duplicate, "0.13.0").is_err());
    }
    for date in [
        "1900-02-29",
        "2026-02-29",
        "2026-04-31",
        "2026-13-01",
        "2026-10-05\n",
        "0000-01-01",
    ] {
        assert!(fixture.repo.validate_date(date).is_err());
    }
    assert!(fixture.repo.validate_date("2000-02-29").is_ok());
    assert!(fixture.repo.validate_date("2024-02-29").is_ok());
}

#[test]
fn empty_numbered_notes_prepare_without_weakening_release_identity() {
    let fixture = Fixture::new();
    for path in ["releases/storage/CHANGELOG.md", "releases/storage/0.13.md"] {
        let text =
            "# Notes\n\n## [0.13.0]\n\n## [0.12.3] - 2026-10-04\n\nHistory without a final newline";
        fixture.set_source(path, text);
    }
    fixture.prepare().unwrap();
    fixture.stage().unwrap();
    fixture.commit().unwrap();
    fixture.repo.publish(true).unwrap();
    for path in ["releases/storage/CHANGELOG.md", "releases/storage/0.13.md"] {
        assert_eq!(
            fs::read_to_string(fixture.repo.root.join(path)).unwrap(),
            "# Notes\n\n## [0.13.0] - 2026-10-05\n\n## [0.12.3] - 2026-10-04\n\nHistory without a final newline"
        );
    }
}

#[test]
fn shared_note_finalization_preserves_history_and_refuses_conflicting_targets() {
    for path in ["releases/storage/CHANGELOG.md", "releases/storage/0.13.md"] {
        let fixture = Fixture::new();
        let edited = format!(
            "{}\nAdditional reviewed notes.\n",
            fixture.repo.exec.state.borrow().git.source[path]
        );
        fs::write(fixture.repo.root.join(path), &edited).unwrap();
        for staged in [false, true] {
            fixture.repo.exec.state.borrow_mut().git.staged = staged;
            let error = fixture
                .repo
                .preflight(&fixture.selection)
                .unwrap_err()
                .to_string();
            assert!(error.starts_with(&format!(
                "{path} does not match the admitted release bytes for source source"
            )));
            assert!(error.contains("git add alone is insufficient"));
            assert_eq!(
                fs::read_to_string(fixture.repo.root.join(path)).unwrap(),
                edited
            );
            assert!(!fixture.repo.exec.state.borrow().has_call(|call| {
                matches!(
                    call[0].as_str(),
                    "make" | "cargo" | "write" | "stream-write"
                )
            }));
        }
        // Substitute committing the notes into the selected source. No real
        // commit, staging or release effects are performed by this fixture.
        fixture
            .repo
            .exec
            .state
            .borrow_mut()
            .git
            .source
            .insert(path.to_owned(), edited);
        fixture.repo.preflight(&fixture.selection).unwrap();
    }

    for notes in [
        "##\t[0.13.0]  \n\nChanges.\n\n## [0.12.3] - 2026-10-04\nHistory",
        "## [0.13.0] - Unreleased\n\nChanges.\n\n## [0.12.3] - 2026-10-04\nHistory",
        "## [0.13.0]",
        "## [0.12.3]\n\nUndated published history",
    ] {
        let fixture = Fixture::new();
        for path in ["releases/storage/CHANGELOG.md", "releases/storage/0.13.md"] {
            fixture.set_source(path, notes);
        }
        fixture.repo.preflight(&fixture.selection).unwrap();
        fixture.release();
        fixture.repo.publish(true).unwrap();
        for path in ["releases/storage/CHANGELOG.md", "releases/storage/0.13.md"] {
            let finalized = fs::read_to_string(fixture.repo.root.join(path)).unwrap();
            assert!(finalized.starts_with("## [0.13.0] - 2026-10-05\n"));
            if let Some(history) = notes.find("## [0.12.3]") {
                assert!(finalized.ends_with(&notes[history..]));
            }
        }
    }

    for conflict in [
        "##\t[0.13.1]  \nDifferent candidate\n",
        "## [0.13.0]\n\n##\t[0.13.0]\t-\t2026-10-04  \nAlready released\n",
    ] {
        let fixture = Fixture::new();
        fixture.set_source("releases/storage/CHANGELOG.md", conflict);
        assert!(fixture.repo.preflight(&fixture.selection).is_err());
        assert!(fixture.prepare().is_err());
        fixture.assert_original();
    }
}

#[test]
fn failed_shared_note_finalization_never_installs_partial_output() {
    let fixture = Fixture::new();
    let finalizer = fixture.repo.root.join(CHANGELOG_FINALIZER);
    fs::write(&finalizer, "BEGIN { print \"partial output\"; exit 23 }").unwrap();
    let error = fixture.prepare().unwrap_err();
    let Some(ToolError::Execution(failure)) = error.downcast_ref::<ToolError>() else {
        panic!("expected the original typed finalizer failure: {error}");
    };
    assert_eq!(failure.evidence.status.unwrap().code(), Some(23));
    assert_eq!(failure.evidence.stdout, b"partial output\n");
    fixture.assert_original();
    fs::remove_file(finalizer).unwrap();
    assert!(fixture.prepare().is_err());
    fixture.assert_original();
}

#[test]
fn all_release_kinds_finalize_root_and_detail_notes_at_saved_date() {
    for (kind, version, detail) in [
        ("patch", "0.12.4", "releases/storage/0.12.md"),
        ("minor", "0.13.0", "releases/storage/0.13.md"),
        ("major", "1.0.0", "releases/storage/1.0.md"),
    ] {
        let mut fixture = Fixture::new();
        fixture.selection.kind = kind.to_owned();
        fixture.selection.version = version.to_owned();
        fixture.select_detail(detail);
        for path in ["releases/storage/CHANGELOG.md", detail] {
            let text = format!("# Notes\n\n## [{version}]\n\nChanges.\n\n## 0.12.3\n\nHistory.\n");
            fixture.set_source(path, &text);
        }
        fixture.prepare().unwrap();
        assert_eq!(fixture.repo.version(None).unwrap(), version);
        let files = fixture.repo.surfaces(&fixture.selection).unwrap();
        assert_eq!(
            files.keys().map(String::as_str).collect::<Vec<_>>(),
            [
                "Cargo.lock",
                "Cargo.toml",
                "crates/ic-storage/Cargo.toml",
                "qualification/Cargo.lock",
                "qualification/Cargo.toml",
                detail,
                "releases/storage/CHANGELOG.md"
            ],
        );
        for path in ["releases/storage/CHANGELOG.md", detail] {
            let text = fs::read_to_string(fixture.repo.root.join(path)).unwrap();
            assert!(text.contains(&format!("## [{version}] - 2026-10-05")));
            assert!(text.ends_with("## 0.12.3\n\nHistory.\n"));
        }
        assert!(
            fs::read_to_string(fixture.repo.root.join("Cargo.lock"))
                .unwrap()
                .contains("name = \"other\"\nversion = \"0.12.3\"")
        );
    }
}

#[test]
fn invalid_prepared_artifacts_stop_final_qualification_without_replacing_evidence() {
    for changed in ["head", "missing", "corrupted"] {
        let fixture = Fixture::new();
        fixture.release();
        let prepared_path = fixture.repo.receipt_path("0.13.0", Prepared).unwrap();
        let mut prepared: PackageEvidence =
            serde_json::from_slice(&fs::read(&prepared_path).unwrap()).unwrap();
        let retained = fixture
            .repo
            .retained_package(&prepared.archives["ic-storage"])
            .unwrap();
        match changed {
            "head" => {
                prepared.package_head = "unvalidated-head".to_owned();
                fs::write(&prepared_path, serde_json::to_vec(&prepared).unwrap()).unwrap();
            }
            "missing" => fs::remove_file(&retained).unwrap(),
            "corrupted" => fs::write(&retained, "corrupted prepared archive").unwrap(),
            _ => unreachable!(),
        }
        let final_path = fixture.repo.receipt_path("0.13.0", Final).unwrap();
        let final_bytes = fs::read(&final_path).unwrap();
        let final_evidence: PackageEvidence = serde_json::from_slice(&final_bytes).unwrap();
        let final_archive = fixture
            .repo
            .retained_package(&final_evidence.archives["ic-storage"])
            .unwrap();
        let archive_bytes = fs::read(&final_archive).unwrap();
        let working_archive = fixture.repo.package_path("ic-storage", "0.13.0").unwrap();
        let working_bytes = fs::read(&working_archive).unwrap();
        let prepared_bytes = fs::read(&prepared_path).unwrap();
        let call_start = fixture.repo.exec.state.borrow().calls.len();

        assert!(fixture.repo.qualify_release().is_err(), "{changed}");
        assert_eq!(fs::read(&final_path).unwrap(), final_bytes);
        assert_eq!(fs::read(&final_archive).unwrap(), archive_bytes);
        assert_eq!(fs::read(&working_archive).unwrap(), working_bytes);
        assert_eq!(fs::read(&prepared_path).unwrap(), prepared_bytes);
        assert!(
            !fixture.repo.exec.state.borrow().calls[call_start..]
                .iter()
                .any(|call| is_package_call(call))
        );
    }
}

#[test]
fn head_change_during_final_packaging_preserves_prepared_evidence() {
    let fixture = Fixture::new();
    fixture.prepare().unwrap();
    fixture.stage().unwrap();
    let path = fixture.repo.receipt_path("0.13.0", Prepared).unwrap();
    let prepared_bytes = fs::read(&path).unwrap();
    let prepared: PackageEvidence = serde_json::from_slice(&prepared_bytes).unwrap();
    let archive = fixture
        .repo
        .retained_package(&prepared.archives["ic-storage"])
        .unwrap();
    let archive_bytes = fs::read(&archive).unwrap();
    fixture.fail_build(BuildFault::PackageHead);
    assert!(fixture.commit().is_err());
    assert!(fixture.repo.exec.state.borrow().git.descendant.is_some());
    assert!(!fixture.repo.exec.state.borrow().git.tag);
    assert!(!fixture.repo.receipt_path("0.13.0", Final).unwrap().exists());
    assert_eq!(fs::read(&path).unwrap(), prepared_bytes);
    assert_eq!(fs::read(&archive).unwrap(), archive_bytes);
}

#[test]
fn selected_older_release_reuses_exact_receipts_and_archives_after_a_fix() {
    let fixture = Fixture::new();
    fixture.release();
    let prepared_path = fixture.repo.receipt_path("0.13.0", Prepared).unwrap();
    let final_path = fixture.repo.receipt_path("0.13.0", Final).unwrap();
    let prepared_bytes = fs::read(&prepared_path).unwrap();
    let final_bytes = fs::read(&final_path).unwrap();
    let evidence: PackageEvidence = serde_json::from_slice(&final_bytes).unwrap();
    let archive = fixture
        .repo
        .retained_package(&evidence.archives["ic-storage"])
        .unwrap();
    let archive_bytes = fs::read(&archive).unwrap();
    let mut descendant = {
        let state = fixture.repo.exec.state.borrow();
        state.git.release.clone().unwrap()
    };
    descendant.insert(
        "source.rs".to_owned(),
        "// committed callback fix\n".to_owned(),
    );
    for (path, text) in &descendant {
        fs::write(fixture.repo.root.join(path), text).unwrap();
    }
    fixture.repo.exec.state.borrow_mut().git.descendant = Some(descendant);
    fs::write(
        fixture.repo.package_path("ic-storage", "0.13.0").unwrap(),
        "newer working archive",
    )
    .unwrap();
    let calls = fixture.repo.exec.state.borrow().calls.len();
    fixture
        .repo
        .selected_committed_check(&fixture.selection, "release")
        .unwrap();
    fixture
        .repo
        .tagged_check(&fixture.selection, "release")
        .unwrap();
    fixture
        .repo
        .push_check(&fixture.selection, "release")
        .unwrap();
    assert!(fixture.repo.release_commit().is_err());
    assert!(
        fixture
            .repo
            .selected_committed_check(&fixture.selection, "fix")
            .is_err()
    );
    assert_eq!(fs::read(&prepared_path).unwrap(), prepared_bytes);
    assert_eq!(fs::read(&final_path).unwrap(), final_bytes);
    assert_eq!(fs::read(&archive).unwrap(), archive_bytes);
    assert!(
        !fixture.repo.exec.state.borrow().calls[calls..]
            .iter()
            .any(|call| {
                call[0] == "make"
                    || is_package_call(call)
                    || is_cargo_call(call, "publish")
                    || is_cargo_call(call, "fetch")
            })
    );

    for conflict in [GitFault::History, GitFault::Parent, GitFault::TagCommit] {
        fixture.fail_git(conflict.clone());
        assert!(
            fixture
                .repo
                .push_check(&fixture.selection, "release")
                .is_err(),
            "{conflict:?}"
        );
        assert_eq!(fs::read(&final_path).unwrap(), final_bytes);
    }
    fixture.repo.exec.state.borrow_mut().clear_faults();
    fs::write(&archive, "corrupted retained archive").unwrap();
    assert!(
        fixture
            .repo
            .push_check(&fixture.selection, "release")
            .is_err()
    );
    fs::write(&archive, archive_bytes).unwrap();
    fs::remove_file(&final_path).unwrap();
    let calls = fixture.repo.exec.state.borrow().calls.len();
    assert!(
        fixture
            .repo
            .selected_committed_check(&fixture.selection, "release")
            .is_err()
    );
    assert!(!final_path.exists());
    assert!(
        !fixture.repo.exec.state.borrow().calls[calls..]
            .iter()
            .any(|call| is_package_call(call))
    );
}

#[test]
fn policy_drift_during_projection_refuses_before_metadata_publication() {
    let fixture = Fixture::new();
    let finalizer = fixture
        .repo
        .text(CHANGELOG_FINALIZER, Some("source"))
        .unwrap();
    fixture.set_source(
        CHANGELOG_FINALIZER,
        &format!("BEGIN {{ print \"\" >> \"ci/cargo-release-groups.json\" }}\n{finalizer}"),
    );
    let policy = fixture.repo.text(GROUP_POLICY, None).unwrap();
    assert_eq!(
        fixture
            .repo
            .surfaces(&fixture.selection)
            .err()
            .expect("policy drift must refuse")
            .to_string(),
        "release group policy changed",
    );
    fixture.assert_original();
    let changed = fixture.repo.text(GROUP_POLICY, None).unwrap();
    assert_ne!(changed, policy);
    assert!(changed.starts_with(&policy));
}
