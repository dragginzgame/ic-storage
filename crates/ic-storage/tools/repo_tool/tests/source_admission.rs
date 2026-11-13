//! Source admission release contracts.

use super::*;

#[test]
fn validation_refuses_make_modes_before_gates_and_preserves_evidence() {
    let fixture = Fixture::new();
    fixture.repo.verify(&fixture.selection).unwrap();
    let receipt = fixture.repo.validation_path("0.13.0").unwrap();
    let previous = fs::read(&receipt).unwrap();
    fixture.fail_build(BuildFault::MakeExecution);
    fixture.repo.exec.state.borrow_mut().calls.clear();
    assert!(fixture.repo.verify(&fixture.selection).is_err());
    assert_eq!(fs::read(&receipt).unwrap(), previous);
    assert_eq!(
        fixture.repo.exec.state.borrow().calls,
        vec![vec![
            "bash",
            "vendor/toko-tooling/scripts/ci/check-make-execution.sh"
        ]]
    );
    assert!(!receipt.parent().unwrap().join("attempts").exists());
    fixture.assert_original();
}

#[test]
fn validation_binds_saved_intent_and_retains_previous_attempts() {
    let fixture = Fixture::new();
    assert!(fixture.repo.prepare_selected(&fixture.selection).is_err());
    fixture.assert_original();
    fixture.repo.verify(&fixture.selection).unwrap();
    let receipt = fixture.repo.validation_path("0.13.0").unwrap();
    let first = fs::read(&receipt).unwrap();
    for field in [
        "kind", "version", "previous", "source", "date", "remote", "branch",
    ] {
        let mut selection = serde_json::to_value(&fixture.selection).unwrap();
        selection[field] = serde_json::json!("changed");
        let selection = serde_json::from_value(selection).unwrap();
        assert!(
            fixture.repo.prepare_selected(&selection).is_err(),
            "{field}"
        );
        fixture.assert_original();
        assert_eq!(fs::read(&receipt).unwrap(), first);
    }
    fixture.fail_build(BuildFault::Validate);
    assert!(fixture.repo.verify(&fixture.selection).is_err());
    assert_eq!(fs::read(&receipt).unwrap(), first);
    fixture.repo.exec.state.borrow_mut().clear_faults();
    fixture.repo.verify(&fixture.selection).unwrap();
    let attempts = receipt.parent().unwrap().join("attempts");
    let entries: Vec<_> = fs::read_dir(attempts).unwrap().collect();
    assert_eq!(entries.len(), 1);
    assert_eq!(
        fs::read(entries[0].as_ref().unwrap().path()).unwrap(),
        first
    );
    fixture.assert_original();
}

#[test]
fn validation_lock_cannot_be_rebound_to_an_uncommitted_graph() {
    let fixture = Fixture::new();
    fixture.repo.verify(&fixture.selection).unwrap();
    let path = fixture.repo.validation_path("0.13.0").unwrap();
    let mut evidence: ValidationEvidence =
        serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    let changed = fixture_lock().replace(
        "name = \"other\"\nversion = \"0.12.3\"",
        "name = \"other\"\nversion = \"0.12.4\"",
    );
    evidence.lock = changed.as_bytes().to_vec();
    fs::write(&path, serde_json::to_vec(&evidence).unwrap()).unwrap();
    fs::write(fixture.repo.root.join("Cargo.lock"), &changed).unwrap();
    assert!(fixture.repo.prepare_selected(&fixture.selection).is_err());
    assert_eq!(fixture.repo.version(None).unwrap(), "0.12.3");
    assert_eq!(
        fs::read_to_string(fixture.repo.root.join("Cargo.lock")).unwrap(),
        changed
    );
    assert!(
        !fixture
            .repo
            .exec
            .state
            .borrow()
            .has_call(|call| is_cargo_call(call, "fetch") || is_package_call(call))
    );
}

#[test]
fn source_admission_reports_actual_status_and_preserves_git_failures() {
    let fixture = Fixture::new();
    let checkout = fixture.repo.root.join("status-checkout");
    Processes
        .capture(
            &fixture.repo.root,
            "git",
            &[
                "clone",
                "--quiet",
                "--shared",
                Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("../..")
                    .to_str()
                    .unwrap(),
                checkout.to_str().unwrap(),
            ],
        )
        .unwrap();
    let repo = Repository {
        group: "storage".to_owned(),
        root: checkout,
        exec: Processes,
    };
    // Discard stat-cache entries without changing the tree, so a plain status
    // observation would refresh the index even though the checkout is clean.
    repo.git(&["read-tree", "HEAD"]).unwrap();
    repo.git(&["checkout-index", "--all", "--force"]).unwrap();
    let clean_index = fs::read(repo.root.join(".git/index")).unwrap();
    repo.clean().unwrap();
    assert_eq!(fs::read(repo.root.join(".git/index")).unwrap(), clean_index);
    let lock = format!(
        "{}\n# staged fixture\n",
        repo.text("Cargo.lock", None).unwrap()
    );
    fs::write(repo.root.join("Cargo.lock"), &lock).unwrap();
    repo.git(&["add", "--force", "--", "Cargo.lock"]).unwrap();
    let manifest = format!(
        "{}\n# working fixture\n",
        repo.text("Cargo.toml", None).unwrap()
    );
    fs::write(repo.root.join("Cargo.toml"), &manifest).unwrap();
    let untracked = "untracked\nfile";
    fs::write(repo.root.join(untracked), b"untracked fixture").unwrap();
    let tree = repo.git(&["write-tree"]).unwrap();
    let status = repo
        .output(
            "git",
            &[
                "--no-optional-locks",
                "status",
                "--porcelain",
                "--untracked-files=all",
            ],
        )
        .unwrap();
    assert!(status.contains("M  Cargo.lock"));
    assert!(status.contains(" M Cargo.toml"));
    assert!(status.contains("?? \"untracked\\nfile\""));
    let dirty_index = fs::read(repo.root.join(".git/index")).unwrap();
    let error = repo.clean().unwrap_err().to_string();
    assert!(error.ends_with(status.trim_end()));
    assert_eq!(fs::read(repo.root.join(".git/index")).unwrap(), dirty_index);
    assert_eq!(repo.git(&["write-tree"]).unwrap(), tree);
    assert_eq!(repo.text("Cargo.lock", None).unwrap(), lock);
    assert_eq!(repo.text("Cargo.toml", None).unwrap(), manifest);
    assert_eq!(
        fs::read(repo.root.join(untracked)).unwrap(),
        b"untracked fixture"
    );

    fixture.fail_git(GitFault::Status);
    assert_eq!(
        fixture.repo.clean().unwrap_err().to_string(),
        "fixture Git status observation failure"
    );
    assert_eq!(
        fixture.repo.exec.state.borrow().calls,
        [vec![
            "git",
            "--no-optional-locks",
            "status",
            "--porcelain",
            "--untracked-files=all"
        ]]
    );
    assert_eq!(fixture.repo.version(None).unwrap(), "0.12.3");
}

#[test]
fn release_admission_rejects_whitespace_only_paths() {
    let fixture = Fixture::new();
    let repo = Repository {
        group: "storage".to_owned(),
        root: fixture.repo.root.clone(),
        exec: Processes,
    };
    fs::create_dir_all(repo.root.join("scripts/ci")).unwrap();
    for path in [
        "vendor/toko-tooling/scripts/ci/next-release-version.sh",
        "vendor/toko-tooling/scripts/ci/validate-release-inputs.pl",
        "vendor/toko-tooling/scripts/ci/StableVersion.pm",
    ] {
        fs::copy(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../..")
                .join(path),
            repo.root.join(path),
        )
        .unwrap();
    }
    fs::write(repo.root.join(" "), b"original tracked bytes\n").unwrap();
    repo.git(&["init", "--quiet"]).unwrap();
    repo.git(&["add", "--", "."]).unwrap();
    let selection = ReleaseSelection {
        source: repo.git(&["write-tree"]).unwrap(),
        ..fixture.selection.clone()
    };
    let surfaces = repo.surfaces(&selection).unwrap();
    repo.check_surfaces(
        &selection,
        &repo.surfaces(&selection).unwrap(),
        None,
        SurfaceState::OriginalOrCandidate,
    )
    .unwrap();
    repo.check_index(&selection, &surfaces).unwrap();

    fs::write(repo.root.join("  "), b"retained untracked bytes\n").unwrap();
    assert_eq!(
        repo.check_surfaces(
            &selection,
            &repo.surfaces(&selection).unwrap(),
            None,
            SurfaceState::OriginalOrCandidate
        )
        .unwrap_err()
        .to_string(),
        "release candidate has untracked files"
    );
    assert_eq!(repo.git(&["write-tree"]).unwrap(), selection.source);
    assert_eq!(
        fs::read(repo.root.join("  ")).unwrap(),
        b"retained untracked bytes\n"
    );
    fs::remove_file(repo.root.join("  ")).unwrap();

    fs::write(repo.root.join(" "), b"changed tracked bytes\n").unwrap();
    assert_eq!(
        repo.check_surfaces(
            &selection,
            &repo.surfaces(&selection).unwrap(),
            None,
            SurfaceState::OriginalOrCandidate
        )
        .unwrap_err()
        .to_string(),
        "release candidate contains changes outside its version surfaces"
    );
    assert_eq!(repo.git(&["write-tree"]).unwrap(), selection.source);
    assert_eq!(
        fs::read(repo.root.join(" ")).unwrap(),
        b"changed tracked bytes\n"
    );

    repo.git(&["add", "--", " "]).unwrap();
    let staged = repo.git(&["write-tree"]).unwrap();
    fs::write(repo.root.join(" "), b"original tracked bytes\n").unwrap();
    repo.check_surfaces(
        &selection,
        &repo.surfaces(&selection).unwrap(),
        None,
        SurfaceState::OriginalOrCandidate,
    )
    .unwrap();
    assert_eq!(
        repo.check_index(&selection, &surfaces)
            .unwrap_err()
            .to_string(),
        "index contains unrelated release changes"
    );
    assert_eq!(repo.git(&["write-tree"]).unwrap(), staged);
    assert_eq!(
        fs::read(repo.root.join(" ")).unwrap(),
        b"original tracked bytes\n"
    );
}

#[test]
fn dirty_sources_and_changed_validation_inputs_prevent_version_edits() {
    let fixture = Fixture::new();
    fs::write(fixture.repo.root.join(PACKAGE_README), "unfinished").unwrap();
    assert!(fixture.prepare().is_err());
    assert_eq!(
        fs::read_to_string(fixture.repo.root.join("Cargo.toml")).unwrap(),
        fixture_manifest()
    );
    let fixture = Fixture::new();
    fixture.fail_build(BuildFault::ValidationLock);
    assert!(fixture.prepare().is_err());
    assert_eq!(
        fs::read_to_string(fixture.repo.root.join("Cargo.toml")).unwrap(),
        fixture_manifest()
    );
}

#[test]
fn lockfile_changes_during_post_validation_remote_inspection_stop_before_mutation() {
    let fixture = Fixture::new();
    fixture.fail_git(GitFault::RemoteLock);
    assert!(fixture.prepare().is_err());
    assert_eq!(fixture.repo.version(None).unwrap(), "0.12.3");
    for (path, text) in &fixture.repo.exec.state.borrow().git.source {
        if path == "Cargo.lock" {
            continue;
        }
        assert_eq!(
            fs::read_to_string(fixture.repo.root.join(path)).unwrap(),
            *text
        );
    }
    // Preserve the separately changed lock selection; it is not our version edit.
    assert_eq!(
        fs::read_to_string(fixture.repo.root.join("Cargo.lock")).unwrap(),
        fixture_lock().replace(
            "name = \"other\"\nversion = \"0.12.3\"",
            "name = \"other\"\nversion = \"0.12.4\""
        )
    );
    assert!(
        !fixture
            .repo
            .receipt_path("0.13.0", Prepared)
            .unwrap()
            .exists()
    );
    let state = fixture.repo.exec.state.borrow();
    // Each preflight admits the cache before its final unchanged-lock check.
    // The second check refuses the foreign edit: no third (prepared) admission
    // or archive qualification runs.
    assert_eq!(
        state
            .calls
            .iter()
            .filter(|call| is_cargo_call(call, "fetch"))
            .count(),
        2
    );
    assert!(!state.has_call(is_package_call));
}

#[test]
fn real_index_rejects_hidden_staging_and_checks_exact_prepared_bytes() {
    let fixture = Fixture::new();
    let checkout = fixture.repo.root.join("index-checkout");
    Processes
        .capture(
            &fixture.repo.root,
            "git",
            &[
                "clone",
                "--quiet",
                "--shared",
                "--no-checkout",
                Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("../..")
                    .to_str()
                    .unwrap(),
                checkout.to_str().unwrap(),
            ],
        )
        .unwrap();
    let repo = Repository {
        group: "storage".to_owned(),
        root: checkout,
        exec: Processes,
    };
    let mut selection = fixture.selection.clone();
    selection.source = repo.git(&["rev-parse", "HEAD"]).unwrap();
    repo.git(&["read-tree", "HEAD"]).unwrap();
    // Give this real index fixture a tracked lock in an uncommitted tree object.
    // Existing repository history is reused; no commit or tag is created.
    fs::write(repo.root.join("Cargo.lock"), fixture_lock()).unwrap();
    repo.git(&["add", "--force", "--", "Cargo.lock"]).unwrap();
    selection.source = repo.git(&["write-tree"]).unwrap();
    repo.git(&["read-tree", &selection.source]).unwrap();
    let original = repo.text("README.md", Some("HEAD")).unwrap();
    let expected = format!("{original}\nPrepared release fixture.\n");
    let locked = fixture
        .repo
        .replace_lock_version(&fixture_lock(), "0.12.3", "0.13.0")
        .unwrap();
    let surfaces = BTreeMap::from([
        (
            "README.md".to_owned(),
            ReleaseSurface {
                original: original.clone(),
                expected: expected.clone(),
            },
        ),
        (
            "Cargo.lock".to_owned(),
            ReleaseSurface {
                original: fixture_lock(),
                expected: locked.clone(),
            },
        ),
    ]);
    repo.check_index(&selection, &surfaces).unwrap();

    fs::write(repo.root.join("README.md"), &original).unwrap();
    repo.git(&["update-index", "--chmod=+x", "README.md"])
        .unwrap();
    assert!(repo.check_index(&selection, &surfaces).is_err());
    repo.git(&["read-tree", &selection.source]).unwrap();

    fs::write(repo.root.join("unrelated.rs"), "unrelated index change").unwrap();
    repo.git(&["add", "--", "unrelated.rs"]).unwrap();
    fs::remove_file(repo.root.join("unrelated.rs")).unwrap();
    assert!(repo.check_index(&selection, &surfaces).is_err());
    repo.git(&["read-tree", &selection.source]).unwrap();

    for (staged, accepted) in [
        ("arbitrary staged metadata".to_owned(), false),
        (expected, true),
        (format!("{original} "), false),
    ] {
        fs::write(repo.root.join("README.md"), staged).unwrap();
        repo.git(&["add", "--", "README.md"]).unwrap();
        fs::write(repo.root.join("README.md"), &original).unwrap();
        assert_eq!(repo.check_index(&selection, &surfaces).is_ok(), accepted);
        repo.git(&["read-tree", &selection.source]).unwrap();
    }
    for (staged, accepted) in [
        (locked.clone(), true),
        (
            locked.replace(
                "name = \"other\"\nversion = \"0.12.3\"",
                "name = \"other\"\nversion = \"0.12.4\"",
            ),
            false,
        ),
        (format!("{locked}# arbitrary staged text\n"), false),
    ] {
        fs::write(repo.root.join("Cargo.lock"), staged).unwrap();
        repo.git(&["add", "--force", "--", "Cargo.lock"]).unwrap();
        fs::write(repo.root.join("Cargo.lock"), fixture_lock()).unwrap();
        assert_eq!(repo.check_index(&selection, &surfaces).is_ok(), accepted);
        repo.git(&["read-tree", &selection.source]).unwrap();
    }
}

#[test]
fn patch_preparation_keeps_head_and_rejects_unvalidated_source_or_staging() {
    let mut fixture = Fixture::new();
    let path = "releases/storage/CHANGELOG.md";
    let notes = fixture.repo.exec.state.borrow().git.source[path].replace("[0.13.0]", "[0.12.4]");
    fixture.set_source(path, &notes);
    fixture.selection.kind = "patch".to_owned();
    fixture.selection.version = "0.12.4".to_owned();
    let detail = "releases/storage/0.12.md";
    let notes = "# Detail\n\n## [0.12.4]\n\nFixture changes.\n";
    fixture.set_source(detail, notes);
    fixture.select_detail(detail);
    fixture.prepare().unwrap();
    assert!(fixture.repo.exec.state.borrow().git.release.is_none());
    assert_eq!(fixture.repo.version(None).unwrap(), "0.12.4");
    assert!(!fixture.repo.exec.state.borrow().has_call(|call| {
        call.get(1)
            .is_some_and(|arg| matches!(arg.as_str(), "commit" | "tag" | "push"))
    }));
    let fixture = Fixture::new();
    fixture.prepare().unwrap();
    assert!(fixture.commit().is_err()); // Expected edits are not staged.
    fixture.stage().unwrap();
    fs::write(
        fixture.repo.root.join("source.rs"),
        "// unvalidated source\n",
    )
    .unwrap();
    assert!(fixture.commit().is_err());
    assert!(fixture.repo.exec.state.borrow().git.release.is_none());
    let fixture = Fixture::new();
    fixture.fail_git(GitFault::RemoteAhead);
    assert!(fixture.prepare().is_err());
    fixture.assert_original();
}

#[test]
fn declared_extra_manifest_is_preserved_instead_of_rendered_as_a_member() {
    let fixture = Fixture::new();
    let path = "testing/qualification/Cargo.toml";
    let original = "[workspace.package]\nversion = \"9.8.7\"\n";
    fixture.select_detail(path);
    fixture.set_source(path, original);
    let surfaces = fixture.repo.surfaces(&fixture.selection).unwrap();
    assert_eq!(surfaces[path].original, original);
    assert_eq!(surfaces[path].expected, original);
}
