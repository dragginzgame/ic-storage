//! Actual consumer release phases with substituted Git, gates and uploads.
use super::*;

#[test]
fn qualification_catalog_and_lock_follow_their_owning_group_and_roll_back_together() {
    let catalog_path = "qualification/Cargo.toml";
    let lock_path = "qualification/Cargo.lock";
    let catalog = "[workspace.dependencies]\nic-storage = { path = \"../../crates/ic-storage\", version = \"0.12.3\" }\nic-logging-storage = { path = \"../../crates/ic-logging-storage\", version = \"0.12.3\" }\nic-wire = \"0.12.3\"\n";
    let lock = "version = 4\n[[package]]\nname = \"ic-storage\"\nversion = \"0.12.3\"\n[[package]]\nname = \"ic-logging-storage\"\nversion = \"0.12.3\"\n[[package]]\nname = \"ic-wire\"\nversion = \"0.12.3\"\n";
    for name in ["storage", "metrics-storage", "logging-storage"] {
        let fixture = Fixture::for_group(name);
        let mut policy: serde_json::Value =
            serde_json::from_str(&fixture.repo.text(GROUP_POLICY, None).unwrap()).unwrap();
        policy["shared_metadata_files"] = serde_json::json!([catalog_path, lock_path]);
        fixture.set_source(GROUP_POLICY, &serde_json::to_string(&policy).unwrap());
        let catalog = format!(
            "{catalog}ic-metrics-storage = {{ path = \"../../crates/ic-metrics-storage\", version = \"0.12.3\" }}\n"
        );
        let lock =
            format!("{lock}[[package]]\nname = \"ic-metrics-storage\"\nversion = \"0.12.3\"\n");
        fixture.set_source(catalog_path, &catalog);
        fixture.set_source(lock_path, &lock);
        let original = fixture.repo.exec.state.borrow().git.source.clone();
        let group = fixture.repo.release_group(None).unwrap();
        fixture.fail_files(FileFault::WriteAfter(
            package_manifest(&group.primary).into(),
        ));
        assert!(fixture.prepare().is_err());
        for (path, bytes) in original {
            assert_eq!(fixture.repo.text(&path, None).unwrap(), bytes, "{path}");
        }
        fixture.prepare().unwrap();
        let package = format!("ic-{name}");
        let expected_catalog = {
            catalog
                .lines()
                .map(|line| {
                    if line.starts_with(&format!("{package} =")) {
                        line.replace("0.12.3", "0.13.0")
                    } else {
                        line.to_owned()
                    }
                })
                .collect::<Vec<_>>()
                .join("\n")
                + "\n"
        };
        let expected_lock = {
            lock.replace(
                &format!("name = {package:?}\nversion = \"0.12.3\""),
                &format!("name = {package:?}\nversion = \"0.13.0\""),
            )
        };
        assert_eq!(
            fixture.repo.text(catalog_path, None).unwrap(),
            expected_catalog
        );
        assert_eq!(fixture.repo.text(lock_path, None).unwrap(), expected_lock);
        let files = fixture.repo.release_files(None).unwrap();
        assert!(files.iter().any(|p| p == catalog_path));
        assert!(files.iter().any(|p| p == lock_path));
        fixture.stage().unwrap();
    }
}

#[test]
fn every_group_preserves_unselected_metadata_and_recovers_preparation_and_upload() {
    for name in ["storage", "metrics-storage", "logging-storage"] {
        let fixture = Fixture::for_group(name);
        let group = fixture.repo.release_group(None).unwrap();
        let source = fixture.repo.exec.state.borrow().git.source.clone();
        let failure_path = package_manifest(&group.primary);
        fixture.fail_files(FileFault::WriteAfter(failure_path.into()));
        assert!(fixture.prepare().is_err());
        for (path, original) in &source {
            assert_eq!(
                fs::read_to_string(fixture.repo.root.join(path)).unwrap(),
                *original,
                "{name}: rollback {path}"
            );
        }
        fixture.prepare().unwrap();
        let files = fixture.repo.release_files(None).unwrap();
        let changed = fixture
            .repo
            .exec
            .state
            .borrow()
            .git
            .source
            .iter()
            .filter_map(|(path, original)| {
                (fs::read_to_string(fixture.repo.root.join(path)).unwrap() != *original)
                    .then_some(path.clone())
            })
            .collect::<Vec<_>>();
        assert!(changed.iter().all(|path| files.contains(path)));
        assert_eq!(fixture.repo.version(None).unwrap(), "0.13.0");
        let catalog: toml::Value =
            toml::from_str(&fixture.repo.text("Cargo.toml", None).unwrap()).unwrap();
        for package in packages() {
            let expected = if group.members.iter().any(|p| p == package) {
                "0.13.0"
            } else {
                "0.12.3"
            };
            assert_eq!(
                BuildEffects::package_version(&fixture.repo.root, package).unwrap(),
                expected
            );
            assert_eq!(
                catalog["workspace"]["dependencies"][package]["version"].as_str(),
                Some(expected)
            );
            let owner = fixture
                .repo
                .release_groups(None)
                .unwrap()
                .into_iter()
                .find(|g| g.members.iter().any(|p| p == package))
                .unwrap();
            if owner.name != name {
                assert_eq!(
                    fs::read_to_string(fixture.repo.root.join(&owner.changelog)).unwrap(),
                    source[&owner.changelog]
                );
                assert_eq!(
                    fs::read_to_string(fixture.repo.root.join(package_manifest(package))).unwrap(),
                    source[&package_manifest(package)]
                );
            }
        }
        let receipt = fixture.package_evidence(Prepared);
        assert_eq!(receipt.validation.selection.group, name);
        assert_eq!(receipt.archives.len(), group.members.len());
        assert!(
            group
                .members
                .iter()
                .all(|p| receipt.archives.contains_key(p))
        );
        assert!(
            fixture
                .repo
                .receipt_path("0.13.0", Prepared)
                .unwrap()
                .file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .starts_with(name)
        );
        fixture.stage().unwrap();
        fixture.commit().unwrap();
        fixture.fail_registry(RegistryFault::UploadResponseLost);
        assert!(fixture.repo.publish(false).is_err());
        fixture.repo.exec.state.borrow_mut().registry.fail = None;
        fixture.repo.publish(false).unwrap();
        fixture.repo.publish(false).unwrap();
        assert_eq!(
            fixture.repo.exec.state.borrow().registry.published.len(),
            group.members.len()
        );
    }
}

#[test]
fn core_releases_align_local_requirements_and_preserve_adapter_identity() {
    let fixture = Fixture::new();
    let result = fixture
        .repo
        .project_catalog(&fixture_manifest(), "0.12.3", "0.12.4")
        .unwrap();
    let after: toml::Value = toml::from_str(&result).unwrap();
    assert!(after["workspace"]["package"].get("version").is_none());
    assert_eq!(
        after["workspace"]["dependencies"]["ic-storage"]["version"].as_str(),
        Some("0.12.4")
    );
    let breaking = fixture
        .repo
        .project_catalog(&fixture_manifest(), "0.12.3", "0.13.0")
        .unwrap();
    let after: toml::Value = toml::from_str(&breaking).unwrap();
    assert_eq!(
        after["workspace"]["dependencies"]["ic-storage"]["version"].as_str(),
        Some("0.13.0")
    );
    for prefix in ["", "^", "~", "=", "^  "] {
        let source = fixture_manifest().replace(
            "version = \"0.12.3\"",
            &format!("version = \"{prefix}0.12.3\""),
        );
        for version in ["0.12.4", "0.13.0"] {
            let result = fixture
                .repo
                .project_catalog(&source, "0.12.3", version)
                .unwrap();
            let result: toml::Value = toml::from_str(&result).unwrap();
            assert_eq!(
                result["workspace"]["dependencies"]["ic-storage"]["version"].as_str(),
                Some(format!("{prefix}{version}").as_str())
            );
            assert_eq!(
                result["workspace"]["dependencies"]["other"].as_str(),
                Some("0.12.3")
            );
        }
    }
    for stale in ["0.12", "0.12.2"] {
        let source =
            fixture_manifest().replace("version = \"0.12.3\"", &format!("version = \"{stale}\""));
        assert!(
            fixture
                .repo
                .project_catalog(&source, "0.12.3", "0.12.4")
                .is_err()
        );
    }
    for group in ["metrics-storage", "logging-storage"] {
        let adapter = Fixture::for_group(group);
        let projected = adapter
            .repo
            .project_catalog(&fixture_manifest(), "0.12.3", "0.13.0")
            .unwrap();
        let projected: toml::Value = toml::from_str(&projected).unwrap();
        for name in [
            "ic-storage",
            "ic-storage",
            "ic-metrics-storage",
            "ic-logging-storage",
        ] {
            let expected = if name == format!("ic-{group}") {
                "0.13.0"
            } else {
                "0.12.3"
            };
            assert_eq!(
                projected["workspace"]["dependencies"][name]["version"].as_str(),
                Some(expected)
            );
        }
    }
}

#[test]
fn catalogs_project_owned_packages_without_root_display_aliases() {
    let fixture = Fixture::for_group("metrics-storage");
    let root = "[workspace.dependencies]\nic-storage = { path = \"crates/ic-storage\", version = \"0.12.3\" }\n";
    assert_eq!(
        fixture
            .repo
            .project_catalog(root, "0.12.3", "0.13.0")
            .unwrap(),
        root
    );
    let independent = "[workspace.dependencies]\nmetrics = { package = \"ic-metrics-storage\", path = \"../crates/ic-metrics-storage\", version = \"0.12.3\" }\nother = \"0.12.3\"\n";
    let projected = fixture
        .repo
        .project_catalog(independent, "0.12.3", "0.13.0")
        .unwrap();
    assert_eq!(
        projected,
        independent.replacen("version = \"0.12.3\"", "version = \"0.13.0\"", 1)
    );
    assert!(
        fixture
            .repo
            .project_catalog(
                &independent.replace("path = \"../crates/ic-metrics-storage\", ", ""),
                "0.12.3",
                "0.13.0"
            )
            .is_err()
    );
}

#[test]
fn group_receipts_cannot_be_replayed_under_another_selection() {
    let mut fixture = Fixture::new();
    fixture.prepare().unwrap();
    let receipt = fixture.repo.receipt_path("0.13.0", Prepared).unwrap();
    fixture.repo.group = "metrics-storage".to_owned();
    let foreign = fixture.repo.receipt_path("0.13.0", Prepared).unwrap();
    fs::write(&foreign, fs::read(&receipt).unwrap()).unwrap();
    fixture.selection.group = "metrics-storage".to_owned();
    assert!(fixture.repo.prepared(&fixture.selection).is_err());
    assert_eq!(fs::read(&receipt).unwrap(), fs::read(&foreign).unwrap());
}
