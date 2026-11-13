//! Source responsibilities for the repository adapter.
#[cfg_attr(
    not(test),
    expect(
        clippy::wildcard_imports,
        reason = "Private release submodules share the parent helper vocabulary."
    )
)]
use super::*;
use ic_build::cargo::environment::CompilerOverride;

/// Which source-derived metadata states may be observed during preparation.
#[derive(Clone, Copy, Eq, PartialEq)]
pub(super) enum SurfaceState {
    Candidate,
    OriginalOrCandidate,
}

impl<E: Execute> Repository<E> {
    pub(super) fn clean(&self) -> Result<()> {
        let status = self.output(
            "git",
            &[
                "--no-optional-locks",
                "status",
                "--porcelain",
                "--untracked-files=all",
            ],
        )?;
        require(
            status.trim().is_empty(),
            &format!(
                "working tree/index must be clean; review these Git status entries:\n{}",
                status.trim_end()
            ),
        )
    }

    pub(super) fn configuration(&self) -> Result<BTreeMap<String, String>> {
        let mut configuration = BTreeMap::new();
        for (name, value) in env::vars_os() {
            let name = name.to_str().ok_or("non-UTF-8 environment variable name")?;
            // The receipts identify the declared toolchains. Cargo compiler
            // replacements or wrappers would execute outside those identities.
            require(
                !is_compiler_override(name),
                &format!("unset {name} before release qualification; use the declared toolchains"),
            )?;
            if is_build_variable(name) {
                configuration.insert(
                    format!("env:{name}"),
                    value
                        .into_string()
                        .map_err(|_| "non-UTF-8 build environment")?,
                );
            }
        }
        let cargo_home = env::var_os("CARGO_HOME");
        let home = env::var_os("HOME");
        require(
            cargo_home.is_some() || home.is_some(),
            "set CARGO_HOME or HOME",
        )?;
        // Keep conservative custody of both root names, even when Cargo shadows
        // one. Build owns discovery, include traversal and same-read raw hashes.
        let roots: Vec<_> =
            configuration_directories(&self.root, cargo_home.as_deref(), home.as_deref())?
                .iter()
                .flat_map(|directory| [directory.join("config"), directory.join("config.toml")])
                .filter(|path| path.exists())
                .collect();
        for file in observe_configurations(&roots, CONFIGURATION_LIMITS)? {
            if let Some(field) = file.compiler_overrides.first() {
                let (section, field) = match field {
                    CompilerOverride::Build(field) => ("build", field),
                    CompilerOverride::Environment(name) => ("env", name),
                };
                return Err(format!(
                    "remove {section}.{field} from {} before release qualification; use the declared toolchains",
                    file.path.display()
                ).into());
            }
            // Contents may contain secrets; receipts retain only lookup labels
            // and the hash of the exact bytes parsed by the observer.
            configuration.insert(
                format!("config:{}", file.path.display()),
                file.sha256.to_string(),
            );
        }
        Ok(configuration)
    }

    pub(super) fn check_selection(&self, selection: &ReleaseSelection) -> Result<()> {
        require(
            selection.group == self.group,
            "release group differs from saved intent",
        )?;
        require(
            self.text(GROUP_POLICY, None)? == self.text(GROUP_POLICY, Some(&selection.source))?,
            "release group policy changed",
        )?;
        self.version_parts(&selection.previous)?;
        self.version_parts(&selection.version)?;
        self.validate_date(&selection.date)?;
        require(
            self.output(
                "bash",
                &[
                    "vendor/toko-tooling/scripts/ci/next-release-version.sh",
                    &selection.previous,
                    &selection.kind,
                ],
            )?
            .trim()
                == selection.version,
            "candidate differs from the common version selection",
        )?;
        require(
            self.version(Some(&selection.source))? == selection.previous,
            "saved source version differs from the release selection",
        )
    }

    pub(super) fn member_release_manifest(
        &self,
        revision: Option<&str>,
        name: &str,
    ) -> Result<String> {
        let text = self.text(&package_manifest(name), revision)?;
        let manifest: toml::Value = toml::from_str(&text)?;
        let package = manifest.get("package").ok_or("missing Storage package")?;
        require(
            package.get("name").and_then(toml::Value::as_str) == Some(name),
            "unexpected Storage package",
        )?;
        let owner = self
            .release_groups(revision)?
            .into_iter()
            .find(|g| g.members.iter().any(|member| member == name))
            .ok_or("package has no release group")?;
        require(
            owner.version == "package"
                && package
                    .get("version")
                    .and_then(toml::Value::as_str)
                    .is_some(),
            "unexpected group version source",
        )?;
        Ok(text)
    }

    pub(super) fn surfaces(
        &self,
        selection: &ReleaseSelection,
    ) -> Result<BTreeMap<String, ReleaseSurface>> {
        self.check_selection(selection)?;
        let source = &selection.source;
        let previous = &selection.previous;
        let version = &selection.version;
        let group = self.release_group(Some(source))?;
        let finalizer = self.root.join(CHANGELOG_FINALIZER);
        let mut surfaces = BTreeMap::new();
        for (path, role) in group.surfaces() {
            let original = self.text(&path, Some(source))?;
            let expected = match role {
                groups::SurfaceRole::Lock => {
                    Self::replace_group_lock_version(&group, &original, previous, version)?
                }
                groups::SurfaceRole::Catalog => {
                    self.project_group_catalog(&group, &original, previous, version)?
                }
                groups::SurfaceRole::Member => render_group_manifest(
                    &original,
                    previous,
                    version,
                    ManifestVersion::Package,
                    &[],
                )?,
                groups::SurfaceRole::Notes => self.release_changelog(
                    &finalizer,
                    &original,
                    previous,
                    version,
                    &selection.date,
                )?,
                groups::SurfaceRole::Preserved => original.clone(),
            };
            surfaces.insert(path, ReleaseSurface { original, expected });
        }
        // Do not retain this phase-local observation across helper execution.
        require(
            self.text(GROUP_POLICY, None)? == self.text(GROUP_POLICY, Some(source))?,
            "release group policy changed",
        )?;
        Ok(surfaces)
    }

    pub(super) fn check_surfaces(
        &self,
        selection: &ReleaseSelection,
        surfaces: &BTreeMap<String, ReleaseSurface>,
        revision: Option<&str>,
        state: SurfaceState,
    ) -> Result<()> {
        let mut expected_changes = Vec::new();
        for (path, surface) in surfaces {
            let actual = self.text(path, revision)?;
            require(
                actual == surface.expected
                    || state == SurfaceState::OriginalOrCandidate && actual == surface.original,
                &format!(
                    "{path} does not match the admitted release bytes for source {}. \
                     For a new release, commit intended source edits first; git add alone is insufficient. \
                     For recovery, preserve the saved plan and reconcile the conflicting file",
                    selection.source
                ),
            )?;
            if actual != surface.original {
                expected_changes.push(path.clone());
            }
        }
        let mut args = vec!["diff", "--name-only", &selection.source];
        if let Some(revision) = revision {
            args.push(revision);
        }
        args.push("--");
        require(
            self.output("git", &args)?
                .lines()
                .eq(expected_changes.iter().map(String::as_str)),
            "release candidate contains changes outside its version surfaces",
        )?;
        require(
            self.output("git", &["ls-files", "--others", "--exclude-standard"])?
                .is_empty(),
            "release candidate has untracked files",
        )
    }

    pub(super) fn check_index(
        &self,
        selection: &ReleaseSelection,
        surfaces: &BTreeMap<String, ReleaseSurface>,
    ) -> Result<()> {
        for path in self
            .output(
                "git",
                &["diff", "--cached", "--name-only", &selection.source, "--"],
            )?
            .lines()
        {
            let expected = surfaces
                .get(path)
                .ok_or("index contains unrelated release changes")?;
            require(
                self.git(&[
                    "diff",
                    "--cached",
                    "--summary",
                    &selection.source,
                    "--",
                    path,
                ])?
                .is_empty(),
                "staged release metadata changes file mode or identity",
            )?;
            let actual = self.output("git", &["show", &format!(":{path}")])?;
            require(
                actual == expected.expected || actual == expected.original,
                "staged release metadata differs from saved intent",
            )?;
        }
        Ok(())
    }

    pub(super) fn text(&self, path: &str, revision: Option<&str>) -> Result<String> {
        match revision {
            Some(revision) => self.output("git", &["show", &format!("{revision}:{path}")]),
            None => Ok(fs::read_to_string(self.root.join(path))?),
        }
    }
}
