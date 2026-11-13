//! Cargo and Make substitutes retain their own build failures.
use super::*;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(super) enum BuildFault {
    ConcurrentLockEdit,
    LockAdmission,
    Formatting,
    MakeExecution,
    Metadata,
    Package,
    PackageHead,
    RelativeMetadata,
    ToolCheck,
    ToolSetup,
    Validate,
    ValidationLock,
}

#[derive(Default, Deserialize, Serialize)]
pub(super) struct BuildEffects {
    pub fail: Option<BuildFault>,
}

impl BuildEffects {
    pub(super) fn target(&self, root: &Path) -> Result<PathBuf> {
        match self.fail.as_ref() {
            Some(BuildFault::Metadata) => Err("fixture metadata failure".into()),
            Some(BuildFault::RelativeMetadata) => Ok(PathBuf::from("relative output")),
            _ => Ok(root.join("configured output")),
        }
    }

    pub(super) fn capture(
        &self,
        root: &Path,
        program: &str,
        args: &[&str],
        git: &mut GitState,
    ) -> Result<String> {
        match program {
            "rustc" => Ok(format!("rustc {} (fixture)\n", fixture_toolchain())),
            "cargo" if args.get(1) == Some(&"--version") => {
                Ok(format!("cargo {} (fixture)\n", fixture_toolchain()))
            }
            "cargo" if args.get(1) == Some(&"build") => Self::probe_build(root, args),
            "cargo" if args.get(1) == Some(&"fetch") => self.admit_lock(root, args),
            "cargo" | "bash"
                if args.get(1) == Some(&"package")
                    || args.first() == Some(&"scripts/ci/package-workspace.sh") =>
            {
                self.package(root, git)
            }
            "bash" if args == ["vendor/toko-tooling/scripts/ci/check-make-execution.sh"] => {
                self.gate(BuildFault::MakeExecution, "fixture Make execution refusal")
            }
            "make" if args == ["--no-print-directory", "install-tools"] => {
                self.gate(BuildFault::ToolSetup, "fixture tool setup failure")
            }
            "make" if args == ["--no-print-directory", "tools-check", "format-tools-check"] => self
                .gate(
                    BuildFault::ToolCheck,
                    "fixture offline tool admission failure",
                ),
            "make" if args.get(1) == Some(&"validate-release-plan") => {
                self.gate(BuildFault::Validate, "fixture validation failure")?;
                super::validation::runner_evidence(root, args)?;
                if self.fail == Some(BuildFault::ValidationLock) {
                    fs::write(root.join("Cargo.lock"), "unexpected dependency selection")?;
                }
                Ok(String::new())
            }
            "make"
                if args
                    == [
                        "--no-print-directory",
                        "fmt-check",
                        &format!("VALIDATION_TOOLCHAIN={}", fixture_toolchain()),
                    ] =>
            {
                self.gate(BuildFault::Formatting, "fixture formatting failure")
            }
            _ => Err(format!("unexpected fixture process: {program} {args:?}").into()),
        }
    }

    fn gate(&self, fault: BuildFault, message: &str) -> Result<String> {
        if self.fail == Some(fault) {
            return Err(message.into());
        }
        Ok(String::new())
    }
    fn probe_build(root: &Path, args: &[&str]) -> Result<String> {
        require(
            args[2..8]
                == [
                    "--locked",
                    "--offline",
                    "--profile",
                    "wasm-size",
                    "--target",
                    "wasm32-unknown-unknown",
                ],
            "unexpected probe build options",
        )?;
        // Artifacts already supplied by the budget fixture. Only admit
        // selectors with matching files, so omitting the build cannot pass.
        for selector in args[8..].chunks(2) {
            require(
                selector.len() == 2 && selector[0] == "--example",
                "unexpected probe selector",
            )?;
            require(
                root.join("configured output/wasm32-unknown-unknown/wasm-size/examples")
                    .join(format!("{}.wasm", selector[1].replace('-', "_")))
                    .is_file(),
                "probe selector has no matching artifact",
            )?;
        }
        Ok(String::new())
    }

    pub(super) fn version(root: &Path) -> Result<String> {
        Self::package_version(root, "ic-storage")
    }
    pub(super) fn package_version(root: &Path, name: &str) -> Result<String> {
        toml_string(
            &fs::read_to_string(root.join(package_manifest(name)))?,
            "package",
            "version",
        )
    }
    fn admit_lock(&self, root: &Path, args: &[&str]) -> Result<String> {
        require(
            args[2..] == ["--locked", "--offline"],
            "unlocked cache admission",
        )?;
        let text = fs::read_to_string(root.join("Cargo.lock"))?;
        let lock: toml::Value = toml::from_str(&text)?;
        let entries = lock["package"].as_array().ok_or("missing lock packages")?;
        let mut prepared = false;
        for name in packages() {
            let version = Self::package_version(root, name)?;
            let entry = entries
                .iter()
                .find(|entry| entry["name"].as_str() == Some(name))
                .ok_or("missing locked member")?;
            require(
                entry["version"].as_str() == Some(version.as_str()),
                "incoherent locked member",
            )?;
            prepared |= version != "0.12.3";
        }
        if prepared {
            self.gate(
                BuildFault::LockAdmission,
                "fixture locked cache unavailable",
            )?;
            if self.fail == Some(BuildFault::ConcurrentLockEdit) {
                // A foreign writer is distinct from Cargo's read-only admission.
                fs::write(
                    root.join("Cargo.lock"),
                    text.replace(
                        "name = \"other\"\nversion = \"0.12.3\"",
                        "name = \"other\"\nversion = \"0.12.4\"",
                    ),
                )?;
            }
        }
        Ok(String::new())
    }
    fn package(&self, root: &Path, git: &mut GitState) -> Result<String> {
        let target = root.join("configured output");

        let directory = target.join("package");
        fs::create_dir_all(&directory)?;
        if self.fail.as_ref() == Some(&BuildFault::PackageHead) {
            let mut descendant = git.release.clone().ok_or("missing release")?;
            descendant.insert(
                "source.rs".to_owned(),
                "// fix during packaging\n".to_owned(),
            );
            for (path, text) in &descendant {
                fs::write(root.join(path), text)?;
            }
            git.descendant = Some(descendant);
        }
        // Model Cargo's Git metadata changing after the release commit.
        let bytes = if git.descendant.is_some() {
            "descendant-archive"
        } else if git.release.is_some() {
            "release-archive"
        } else {
            "prepared-archive"
        };
        for name in packages() {
            let manifest = fs::read_to_string(root.join(package_manifest(name)))?;
            let value: toml::Value = toml::from_str(&manifest)?;
            let version = value["package"]["version"]
                .as_str()
                .map(str::to_owned)
                .unwrap_or(Self::version(root)?);
            let prefix = match name {
                "ic-storage" => "",
                "ic-metrics-storage" => "metrics-",
                _ => "logging-",
            };
            fs::write(
                directory.join(format!("{name}-{version}.crate")),
                format!("{prefix}{bytes}"),
            )?;
        }
        if self.fail.as_ref() == Some(&BuildFault::Package) {
            return Err("fixture package failure".into());
        }
        Ok(String::new())
    }
}
