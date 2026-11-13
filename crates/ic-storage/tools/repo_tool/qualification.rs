//! Validation and package receipts bound to selected source.
#[cfg_attr(
    not(test),
    expect(
        clippy::wildcard_imports,
        reason = "Private release submodules share the parent helper vocabulary."
    )
)]
use super::*;

/// Whether the selected lock may contain the admitted workspace version projection.
#[derive(Clone, Copy, Eq, PartialEq)]
pub(super) enum LockState {
    Validated,
    ReleaseProjection,
}

use super::preparation::ReleasePlan;

// Reconstructed from the current admitted toolchain at each phase boundary.
// Receipts retain the exact read-only admission and qualification commands.
pub(super) struct ReleaseCommands {
    pub(super) preparation: [Vec<String>; 3],
    pub(super) final_package: Vec<String>,
}

impl ReleaseCommands {
    fn new(toolchain: &str) -> Self {
        let final_package = vec![
            "bash".to_owned(),
            "scripts/ci/package-workspace.sh".to_owned(),
            toolchain.to_owned(),
        ];
        let mut packaging = final_package.clone();
        packaging.push("--allow-dirty".to_owned());
        let make = |target: &str| {
            vec![
                "make".to_owned(),
                "--no-print-directory".to_owned(),
                target.to_owned(),
                format!("VALIDATION_TOOLCHAIN={toolchain}"),
            ]
        };
        Self {
            preparation: [
                vec![
                    "cargo".to_owned(),
                    format!("+{toolchain}"),
                    "fetch".to_owned(),
                    "--locked".to_owned(),
                    "--offline".to_owned(),
                ],
                make("fmt-check"),
                packaging,
            ],
            final_package,
        }
    }

    fn receipt(self, stage: ReceiptStage) -> Vec<Vec<String>> {
        let mut commands = self.preparation.to_vec();
        if stage == Final {
            commands.push(self.final_package);
        }
        commands
    }
}

impl<E: Execute> Repository<E> {
    pub(super) fn validation_path(&self, version: &str) -> Result<PathBuf> {
        self.version_parts(version)?;
        Ok(self
            .artifact_paths()?
            .validation(&format!("{}-{version}", self.group)))
    }

    pub(super) fn run_command(&self, command: &[String]) -> Result<()> {
        self.run(
            &command[0],
            &command[1..].iter().map(String::as_str).collect::<Vec<_>>(),
        )
    }

    pub(super) fn preflight(&self, selection: &ReleaseSelection) -> Result<()> {
        self.configuration()?;
        self.check_selection(selection)?;
        require(
            self.git(&["rev-parse", "HEAD"])? == selection.source,
            "source HEAD differs from the saved release",
        )?;
        require(
            self.version(None)? == selection.previous,
            "preflight requires the saved base version",
        )?;
        let surfaces = self.surfaces(selection)?;
        self.check_surfaces(
            selection,
            &surfaces,
            None,
            SurfaceState::OriginalOrCandidate,
        )?;
        self.check_index(selection, &surfaces)?;
        let lock = fs::read(self.root.join("Cargo.lock"))?;
        let interrupted = lock != self.text("Cargo.lock", Some(&selection.source))?.as_bytes();
        if interrupted {
            // The exact candidate lock can precede the manifest after an
            // interrupted write. Reuse source-bound successful validation;
            // locked Cargo fetching cannot reconcile that temporary mismatch.
            self.validated(selection)?;
        } else {
            self.check_lock_update(
                &lock,
                std::str::from_utf8(&lock)?,
                &selection.previous,
                &selection.previous,
            )?;
        }
        self.remote_ready(selection)?;
        // Remote inspection and offline cache verification cannot change the
        // tracked lockfile whose selection will qualify this attempt.
        if !interrupted {
            self.run(
                "cargo",
                &[
                    &format!("+{}", self.toolchain()?),
                    "fetch",
                    "--locked",
                    "--offline",
                ],
            )?;
            // Only coherent, admitted release sources may prepare tools. The
            // complete common selection is required by the offline full gate.
            self.run("make", &["--no-print-directory", "install-tools"])?;
        }
        self.run(
            "make",
            &["--no-print-directory", "tools-check", "format-tools-check"],
        )?;
        require(
            fs::read(self.root.join("Cargo.lock"))? == lock,
            "dependency selection changed during preflight",
        )?;
        require(
            self.git(&["rev-parse", "HEAD"])? == selection.source,
            "source changed during preflight",
        )?;
        self.check_surfaces(
            selection,
            &surfaces,
            None,
            SurfaceState::OriginalOrCandidate,
        )?;
        self.check_index(selection, &surfaces)
    }

    pub(super) fn verify(&self, selection: &ReleaseSelection) -> Result<()> {
        self.run(
            "bash",
            &["vendor/toko-tooling/scripts/ci/check-make-execution.sh"],
        )?;
        let configuration = self.configuration()?;
        self.check_selection(selection)?;
        self.clean()?;
        require(
            self.git(&["rev-parse", "HEAD"])? == selection.source
                && self.version(None)? == selection.previous,
            "validation requires the saved source and base version",
        )?;
        let lock = fs::read(self.root.join("Cargo.lock"))?;
        self.check_lock_update(
            &lock,
            std::str::from_utf8(&lock)?,
            &selection.previous,
            &selection.previous,
        )?;
        let identities = self.identities()?;
        let (plan_path, plan, context) = self.create_validation_plan(selection)?;
        let command = validation_run::command(&self.toolchain()?, &plan_path, selection);
        self.run_command(&command)?;
        let group_validation = self.collect_validation(selection, &plan_path, plan, context)?;
        let evidence = ValidationEvidence {
            schema: 2,
            group_validation,
            selection: selection.clone(),
            lock,
            identities,
            configuration,
            command,
        };
        self.clean()?;
        require(
            self.git(&["rev-parse", "HEAD"])? == selection.source,
            "source changed during validation",
        )?;
        self.verify_inputs(&evidence, LockState::Validated)?;
        let path = self.validation_path(&selection.version)?;
        let stale_prepared = self.superseded_preparation(selection)?;
        self.retain_receipt(&path, "validated", &selection.version)?;
        if let Some(receipt) = &stale_prepared {
            self.retain_receipt(receipt, "prepared", &selection.version)?;
        }
        // Fresh successful validation supersedes only an admitted older source.
        // Preserve both receipts first; unchanged-source recovery keeps its active
        // prepared receipt and existing integrity checks.
        if let Some(receipt) = stale_prepared {
            fs::remove_file(receipt)?;
        }
        self.exec.write_json(&path, &evidence)
    }

    fn superseded_preparation(&self, selection: &ReleaseSelection) -> Result<Option<PathBuf>> {
        let path = self.receipt_path(&selection.version, Prepared)?;
        if !path.exists() {
            return Ok(None);
        }
        let prepared: PackageEvidence = evidence::read_receipt(&path)?;
        let previous = &prepared.validation.selection;
        if previous.source == selection.source {
            return Ok(None);
        }
        require(
            previous.group == selection.group
                && previous.kind == selection.kind
                && previous.previous == selection.previous
                && previous.version == selection.version
                && previous.remote == selection.remote
                && previous.branch == selection.branch,
            "superseded preparation identifies another release or destination",
        )?;
        require(
            prepared.package_head == previous.source,
            "superseded package evidence differs from its preparation source",
        )?;
        self.git(&[
            "merge-base",
            "--is-ancestor",
            &previous.source,
            &selection.source,
        ])?;
        Ok(Some(path))
    }

    fn retain_receipt(&self, receipt: &Path, kind: &str, version: &str) -> Result<()> {
        if receipt.exists() {
            let retained = receipt
                .parent()
                .ok_or("validation path has no parent")?
                .join("attempts")
                .join(format!(
                    "{version}-{kind}.{}.{}.json",
                    std::process::id(),
                    SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos()
                ));
            self.exec
                .write_bytes(&retained, &evidence::receipt_bytes(receipt)?)?;
        }
        Ok(())
    }

    pub(super) fn verify_inputs(
        &self,
        evidence: &ValidationEvidence,
        state: LockState,
    ) -> Result<()> {
        require(
            evidence.lock
                == self
                    .text("Cargo.lock", Some(&evidence.selection.source))?
                    .as_bytes(),
            "validation lockfile differs from the saved source",
        )?;
        require(
            self.configuration()? == evidence.configuration,
            "build configuration differs from release validation",
        )?;
        require(
            self.identities()? == evidence.identities,
            "compiler identities differ from release validation",
        )?;
        require(
            evidence.schema == 2
                && evidence.command
                    == validation_run::command(
                        &self.toolchain()?,
                        &evidence.group_validation.plan_path,
                        &evidence.selection,
                    ),
            "validation evidence has unexpected commands",
        )?;
        self.admit_validation(&evidence.selection, &evidence.group_validation)?;
        self.check_selection(&evidence.selection)?;
        let current = fs::read(self.root.join("Cargo.lock"))?;
        if current != evidence.lock {
            require(
                state == LockState::ReleaseProjection,
                "selected lockfile differs from release validation",
            )?;
            self.check_lock_update(
                &evidence.lock,
                std::str::from_utf8(&current)?,
                &evidence.selection.previous,
                &evidence.selection.version,
            )?;
        }
        Ok(())
    }

    pub(super) fn validated(&self, selection: &ReleaseSelection) -> Result<ValidationEvidence> {
        let evidence: ValidationEvidence =
            evidence::read_receipt(&self.validation_path(&selection.version)?)?;
        require(
            evidence.selection == *selection,
            "validation identifies another release intent",
        )?;
        self.verify_inputs(&evidence, LockState::ReleaseProjection)?;
        Ok(evidence)
    }

    pub(super) fn command_plan(&self) -> Result<ReleaseCommands> {
        Ok(ReleaseCommands::new(&self.toolchain()?))
    }

    pub(super) fn write_evidence(
        &self,
        evidence: &PackageEvidence,
        stage: ReceiptStage,
    ) -> Result<()> {
        self.exec.write_json(
            &self.receipt_path(&evidence.validation.selection.version, stage)?,
            evidence,
        )
    }

    pub(super) fn prepare_selected(&self, selection: &ReleaseSelection) -> Result<()> {
        self.validated(selection)?;
        require(
            self.git(&["rev-parse", "HEAD"])? == selection.source,
            "source changed before preparation",
        )?;
        let expected = self.surfaces(selection)?;
        self.check_surfaces(
            selection,
            &expected,
            None,
            SurfaceState::OriginalOrCandidate,
        )?;
        let plan = ReleasePlan::capture(self, &expected)?;
        let result = (|| -> Result<()> {
            plan.publish(self)?;
            // Source-derived candidate bytes remain the same throughout this
            // transaction. Re-admit the live HEAD, receipt and validation inputs
            // after publication, then qualify against this exact projection.
            self.qualify_prepared(self.preparation_validation(selection)?, &expected)?;
            Ok(())
        })();
        result.map_err(|source| plan.rollback(self, selection, source))
    }

    pub(super) fn verify_evidence(
        &self,
        selection: &ReleaseSelection,
        head: &str,
        stage: ReceiptStage,
        location: ArchiveLocation,
    ) -> Result<PackageEvidence> {
        let evidence: PackageEvidence =
            evidence::read_receipt(&self.receipt_path(&selection.version, stage)?)?;
        require(
            evidence.validation.selection == *selection,
            "package evidence identifies another release intent",
        )?;
        self.verify_inputs(&evidence.validation, LockState::ReleaseProjection)?;
        require(
            Self::sha256(&self.root.join("Cargo.lock"))? == evidence.lock_sha256,
            "selected lockfile differs from package qualification",
        )?;
        require(
            evidence.package_head == head,
            "package evidence identifies another Git HEAD",
        )?;
        let mut paths = None;
        let members = self.selected_packages()?;
        require(
            evidence.archives.len() == members.len()
                && members
                    .iter()
                    .all(|name| evidence.archives.contains_key(name)),
            "receipt package selection differs from release group",
        )?;
        for (package, digest) in evidence.archives() {
            // Preserve per-archive refusal order; discover paths only after the
            // first digest is admitted, then reuse them within this phase.
            let identity = digest.parse::<Sha256Digest>()?;
            if paths.is_none() {
                paths = Some(self.artifact_paths()?);
            }
            let paths = paths.as_ref().expect("artifact paths were admitted");
            require(
                Self::sha256(&paths.retained(identity))? == digest,
                "retained archive differs from release validation",
            )?;
            if location == Working {
                require(
                    Self::sha256(&paths.package(package, &selection.version))? == digest,
                    "working package differs from release validation",
                )?;
            }
        }
        let commands = self.command_plan()?.receipt(stage);
        require(
            evidence.commands == commands,
            "package evidence has unexpected commands",
        )?;
        Ok(evidence)
    }

    pub(super) fn prepared(&self, selection: &ReleaseSelection) -> Result<PackageEvidence> {
        let validation = self.preparation_validation(selection)?;
        let surfaces = self.surfaces(selection)?;
        self.qualify_prepared(validation, &surfaces)
    }

    pub(super) fn preparation_validation(
        &self,
        selection: &ReleaseSelection,
    ) -> Result<ValidationEvidence> {
        require(
            self.git(&["rev-parse", "HEAD"])? == selection.source,
            "prepared source changed",
        )?;
        self.validated(selection)
    }

    pub(super) fn qualify_prepared(
        &self,
        validation: ValidationEvidence,
        surfaces: &BTreeMap<String, ReleaseSurface>,
    ) -> Result<PackageEvidence> {
        let selection = &validation.selection;
        self.check_surfaces(selection, surfaces, None, SurfaceState::Candidate)?;
        if self.receipt_path(&selection.version, Prepared)?.exists() {
            // Existing evidence is checked, never silently repaired or replaced.
            return self.verify_evidence(selection, &selection.source, Prepared, Working);
        }
        let commands = self.command_plan()?.preparation;
        let [admission, formatting, packaging] = &commands;
        // The source projection already owns every version edit. Cargo only
        // admits that locked graph and its prepared cache; it never selects one.
        self.run_command(admission)?;
        self.check_lock_update(
            &validation.lock,
            &fs::read_to_string(self.root.join("Cargo.lock"))?,
            &selection.previous,
            &selection.version,
        )?;
        self.run_command(formatting)?;
        let lock_sha256 = Self::sha256(&self.root.join("Cargo.lock"))?;
        self.run_command(packaging)?;
        self.check_surfaces(selection, surfaces, None, SurfaceState::Candidate)?;
        require(
            self.git(&["rev-parse", "HEAD"])? == selection.source,
            "source changed during preparation",
        )?;
        self.verify_inputs(&validation, LockState::ReleaseProjection)?;
        require(
            Self::sha256(&self.root.join("Cargo.lock"))? == lock_sha256,
            "packaging changed selected dependencies",
        )?;
        let head = selection.source.clone();
        let evidence =
            PackageEvidence::record(self, validation, head, lock_sha256, commands.to_vec())?;
        self.write_evidence(&evidence, Prepared)?;
        Ok(evidence)
    }
}
