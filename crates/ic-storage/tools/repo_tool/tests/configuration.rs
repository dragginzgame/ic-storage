//! Configuration release contracts.

use super::*;

#[test]
fn compiler_identity_requires_one_observation_and_matching_policy() {
    let fixture = Fixture::new();
    let identities = fixture.repo.identities().unwrap();
    assert_eq!(identities[1], identities[2]);
    assert_eq!(
        fixture
            .repo
            .exec
            .state
            .borrow()
            .calls
            .iter()
            .filter(|call| call[0] == "rustc")
            .count(),
        1,
    );
    let manifest = fixture_manifest().replace(&fixture_toolchain(), "0.0.0");
    fixture.set_source("Cargo.toml", &manifest);
    assert!(fixture.repo.identities().is_err());
    fixture.set_source("rust-toolchain.toml", "[toolchain]\nchannel = \"0.0.0\"\n");
    assert!(fixture.repo.identities().is_err());
}

#[test]
fn compiler_environment_overrides_refuse_preparation_and_release_effects() {
    const ROOT: &str = "IC_MEMORY_TEST_COMPILER_ROOT";
    const STATE: &str = "IC_MEMORY_TEST_COMPILER_STATE";
    const OVERRIDES: [&str; 8] = [
        "RUSTC",
        "RUSTC_WRAPPER",
        "RUSTC_WORKSPACE_WRAPPER",
        "RUSTDOC",
        "CARGO_BUILD_RUSTC",
        "CARGO_BUILD_RUSTC_WRAPPER",
        "CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER",
        "CARGO_BUILD_RUSTDOC",
    ];
    if let Some(state) = env::var_os(STATE) {
        // Separate processes exercise real inherited environment without
        // unsafe mutation of the concurrent Rust test harness environment.
        let fixture = Fixture::new();
        let receipt = fixture.repo.receipt_path("0.13.0", Prepared).unwrap();
        fs::create_dir_all(receipt.parent().unwrap()).unwrap();
        fs::write(&receipt, "previous evidence").unwrap();
        assert!(fixture.prepare().is_err());
        fixture.assert_original();
        assert_eq!(fs::read(receipt).unwrap(), b"previous evidence");
        assert!(!fixture.repo.exec.state.borrow().has_call(|call| {
            call[0] == "make" || is_cargo_call(call, "fetch") || is_package_call(call)
        }));

        let repo = Repository {
            group: "storage".to_owned(),
            root: PathBuf::from(env::var_os(ROOT).unwrap()),
            exec: Substitute {
                files: RefCell::new(FileEffects::default()),
                state: RefCell::new(
                    serde_json::from_str::<State>(state.to_str().unwrap()).unwrap(),
                ),
            },
        };
        let call_start = repo.exec.state.borrow().calls.len();
        assert!(repo.qualify_release().is_err());
        assert!(repo.push_check(&fixture.selection, "release").is_err());
        assert!(repo.publish(true).is_err());
        assert!(
            !repo.exec.state.borrow().calls[call_start..]
                .iter()
                .any(|call| {
                    call.get(1).is_some_and(|arg| arg == "push")
                        || (is_package_call(call) || is_cargo_call(call, "publish"))
                })
        );
        return;
    }

    let fixture = Fixture::new();
    fixture.release();
    let state = serde_json::to_string(&*fixture.repo.exec.state.borrow()).unwrap();
    let evidence = fixture
        .repo
        .verify_evidence(&fixture.selection, "release", Final, Working)
        .unwrap();
    let paths = [
        fixture.repo.receipt_path("0.13.0", Prepared).unwrap(),
        fixture.repo.receipt_path("0.13.0", Final).unwrap(),
        fixture.repo.package_path("ic-storage", "0.13.0").unwrap(),
        fixture
            .repo
            .retained_package(&evidence.archives["ic-storage"])
            .unwrap(),
    ];
    let before: Vec<_> = paths.iter().map(|path| fs::read(path).unwrap()).collect();
    for name in OVERRIDES {
        for value in ["", "alternate-tool"] {
            let mut child = Command::new(env::current_exe().unwrap());
            child.args(["--exact", "repo_tool::tests::configuration::compiler_environment_overrides_refuse_preparation_and_release_effects"]);
            for variable in OVERRIDES {
                child.env_remove(variable);
            }
            let output = child
                .env(ROOT, &fixture.repo.root)
                .env(STATE, &state)
                .env(name, value)
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{name}={value:?}: {}{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            for (path, bytes) in paths.iter().zip(&before) {
                assert_eq!(fs::read(path).unwrap(), *bytes);
            }
        }
    }
}

#[test]
fn compiler_configuration_overrides_refuse_preparation() {
    for filename in ["config", "config.toml"] {
        for field in [
            "rustc",
            "rustc-wrapper",
            "rustc-workspace-wrapper",
            "rustdoc",
        ] {
            for value in ["", "alternate-tool"] {
                let fixture = Fixture::new();
                let directory = fixture.repo.root.join(".cargo");
                fs::create_dir(&directory).unwrap();
                let path = directory.join(filename);
                let text = format!("[build]\n{field} = {value:?}\n");
                fs::write(&path, &text).unwrap();
                assert!(fixture.prepare().is_err());
                fixture.assert_original();
                assert_eq!(fs::read_to_string(path).unwrap(), text);
                assert!(
                    !fixture
                        .repo
                        .exec
                        .state
                        .borrow()
                        .has_call(|call| call[0] == "make")
                );
            }
        }
    }
    let fixture = Fixture::new();
    let directory = fixture.repo.root.join(".cargo");
    fs::create_dir(&directory).unwrap();
    fs::write(
        directory.join("config.toml"),
        "[build]\njobs = 1\n[profile.dev]\nopt-level = 1\n",
    )
    .unwrap();
    fixture.release();
    fixture.repo.publish(true).unwrap();

    let fixture = Fixture::new();
    let directory = fixture.repo.root.join(".cargo");
    fs::create_dir(&directory).unwrap();
    fs::write(
        directory.join("config.toml"),
        "token = 'private-test-value' trailing\n",
    )
    .unwrap();
    let error = fixture.repo.configuration().unwrap_err().to_string();
    assert!(!error.contains("private-test-value"));
}

#[test]
fn configured_compiler_environment_refuses_before_effects_without_exposing_values() {
    for included in [false, true] {
        for (name, value) in [
            ("RUSTC_WRAPPER", "''"),
            ("RUSTC_WRAPPER", "'private-wrapper-path'"),
            (
                "CARGO_BUILD_RUSTDOC",
                "{ value = 'private-wrapper-path', force = true, relative = true }",
            ),
        ] {
            let fixture = Fixture::new();
            let directory = fixture.repo.root.join(".cargo");
            fs::create_dir(&directory).unwrap();
            let root = directory.join("config.toml");
            let path = if included {
                fs::write(&root, "include = 'compiler.toml'\n").unwrap();
                directory.join("compiler.toml")
            } else {
                root
            };
            let text = format!("[env]\n{name} = {value}\n");
            fs::write(&path, &text).unwrap();
            let error = fixture.prepare().unwrap_err().to_string();
            assert!(error.contains(&format!("env.{name}")), "{error}");
            assert!(error.contains(&path.display().to_string()), "{error}");
            assert!(!error.contains("private-wrapper-path"));
            fixture.assert_original();
            assert_eq!(fs::read_to_string(path).unwrap(), text);
            assert!(!fixture.repo.exec.state.borrow().has_call(|call| {
                call[0] == "make" || is_cargo_call(call, "fetch") || is_package_call(call)
            }));
        }
    }
}

#[test]
fn configuration_budgets_refuse_before_gates_or_metadata_mutation() {
    for budget in ["file", "total", "files", "depth"] {
        let fixture = Fixture::new();
        let directory = fixture.repo.root.join(".cargo");
        fs::create_dir(&directory).unwrap();
        let root = directory.join("config.toml");
        match budget {
            "file" => {
                fs::write(
                    &root,
                    format!("#{}", "x".repeat(CONFIGURATION_LIMITS.file_bytes)),
                )
                .unwrap();
            }
            "total" | "files" => {
                let count = if budget == "total" {
                    5
                } else {
                    CONFIGURATION_LIMITS.files
                };
                let contents = if budget == "total" {
                    format!("#{}", "x".repeat(CONFIGURATION_LIMITS.file_bytes - 1))
                } else {
                    String::new()
                };
                let includes: Vec<_> = (0..count)
                    .map(|index| format!("part-{index}.toml"))
                    .collect();
                fs::write(&root, format!("include = {includes:?}\n")).unwrap();
                for name in includes {
                    fs::write(directory.join(name), &contents).unwrap();
                }
            }
            "depth" => {
                fs::write(&root, "include = 'part-0.toml'\n").unwrap();
                for index in 0..=CONFIGURATION_LIMITS.depth {
                    fs::write(
                        directory.join(format!("part-{index}.toml")),
                        format!("include = 'part-{}.toml'\n", index + 1),
                    )
                    .unwrap();
                }
                fs::write(
                    directory.join(format!("part-{}.toml", CONFIGURATION_LIMITS.depth + 1)),
                    "",
                )
                .unwrap();
            }
            _ => unreachable!(),
        }
        let before = fs::read(&root).unwrap();
        let error = fixture.prepare().unwrap_err();
        assert!(
            error
                .downcast_ref::<ic_build::cargo::environment::ConfigurationError>()
                .is_some(),
            "{budget}: {error}"
        );
        fixture.assert_original();
        assert_eq!(fs::read(root).unwrap(), before);
        assert!(
            !fixture
                .repo
                .exec
                .state
                .borrow()
                .has_call(|call| call[0] == "make")
        );
    }
}

#[test]
fn included_compiler_configuration_refuses_preparation() {
    let fixture = Fixture::new();
    let directory = fixture.repo.root.join(".cargo");
    fs::create_dir(&directory).unwrap();
    fs::write(directory.join("config.toml"), "include = 'compiler.toml'\n").unwrap();
    fs::write(
        directory.join("compiler.toml"),
        "[build]\nrustc-wrapper = 'outside-receipt-compiler'\n",
    )
    .unwrap();
    assert!(fixture.prepare().is_err());
    fixture.assert_original();
    assert!(
        !fixture
            .repo
            .exec
            .state
            .borrow()
            .has_call(|call| { call[0] == "make" || is_cargo_call(call, "fetch") })
    );
}

#[test]
fn included_and_shadowed_configuration_changes_invalidate_release_evidence() {
    let fixture = Fixture::new();
    let directory = fixture.repo.root.join(".cargo");
    fs::create_dir(&directory).unwrap();
    let files = [
        (directory.join("config"), "include = 'shared.toml'\n"),
        (directory.join("config.toml"), "[build]\njobs = 1\n"),
        (directory.join("shared.toml"), "[build]\njobs = 2\n"),
    ];
    for (path, bytes) in &files {
        fs::write(path, bytes).unwrap();
    }
    fixture.release();
    let observed = fixture.repo.configuration().unwrap();
    for (path, bytes) in &files {
        assert_eq!(
            observed[&format!("config:{}", path.display())],
            Sha256Digest::compute(bytes.as_bytes()).to_string()
        );
        // Even a comment-only change must invalidate existing raw-byte custody.
        fs::write(path, format!("{bytes}# changed\n")).unwrap();
        let calls = fixture.repo.exec.state.borrow().calls.len();
        assert!(
            fixture
                .repo
                .push_check(&fixture.selection, "release")
                .is_err()
        );
        assert!(fixture.repo.publish(true).is_err());
        assert!(
            !fixture.repo.exec.state.borrow().calls[calls..]
                .iter()
                .any(|call| {
                    call.get(1).is_some_and(|arg| arg == "push") || is_cargo_call(call, "publish")
                })
        );
        fs::write(path, bytes).unwrap();
        fixture
            .repo
            .verify_evidence(&fixture.selection, "release", Final, Working)
            .unwrap();
    }
}

#[test]
fn inherited_compiler_configuration_refuses_preparation() {
    let fixture = Fixture::new();
    let directory = fixture.repo.root.join(".cargo");
    fs::create_dir(&directory).unwrap();
    fs::write(
        directory.join("config.toml"),
        "[build]\nrustc-wrapper = 'alternate-tool'\n",
    )
    .unwrap();
    let root = fixture.repo.root.join("nested");
    fs::create_dir(&root).unwrap();
    let source = fixture.repo.exec.state.borrow().git.source.clone();
    for path in source
        .keys()
        .map(String::as_str)
        .chain(["rust-toolchain.toml"])
    {
        fs::create_dir_all(root.join(path).parent().unwrap()).unwrap();
        fs::copy(fixture.repo.root.join(path), root.join(path)).unwrap();
    }
    let repo = Repository {
        group: "storage".to_owned(),
        root,
        exec: Substitute {
            files: RefCell::new(FileEffects::default()),
            state: RefCell::new(State {
                git: GitState {
                    source,
                    ..GitState::default()
                },
                ..State::default()
            }),
        },
    };
    assert!(repo.preflight(&fixture.selection).is_err());
    assert_eq!(repo.version(None).unwrap(), "0.12.3");
    assert!(
        !repo
            .exec
            .state
            .borrow()
            .calls
            .iter()
            .any(|call| call[0] == "make")
    );
}
