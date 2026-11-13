//! Group-plan execution and immutable evidence admission; no whole-gate reuse.
#[cfg_attr(
    not(test),
    expect(
        clippy::wildcard_imports,
        reason = "Private release submodules share the parent helper vocabulary."
    )
)]
use super::*;
use validation_plan::{Context, Plan, canonical, regular_path};

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct GroupValidation {
    pub schema: u32,
    pub plan_path: String,
    pub plan: Plan,
    pub context: Context,
    pub runner: String,
    pub proof: serde_json::Value,
    pub files: BTreeMap<String, String>,
}

pub(super) fn command(
    toolchain: &str,
    plan_path: &str,
    selection: &ReleaseSelection,
) -> Vec<String> {
    plan_command(toolchain, plan_path, &selection.source, &selection.group)
}

pub(super) fn plan_command(
    toolchain: &str,
    plan_path: &str,
    source: &str,
    group: &str,
) -> Vec<String> {
    vec![
        "make".to_owned(),
        "--no-print-directory".to_owned(),
        "validate-release-plan".to_owned(),
        format!("VALIDATION_TOOLCHAIN={toolchain}"),
        format!("RELEASE_SOURCE={source}"),
        format!("RELEASE_GROUP={group}"),
        format!("VALIDATION_PLAN={plan_path}"),
        format!("VALIDATION_LOG_DIR={plan_path}.logs"),
    ]
}

fn digest(bytes: &[u8]) -> String {
    Sha256Digest::compute(bytes).to_string()
}

impl<E: Execute> Repository<E> {
    fn plan_bytes(&self, path: &str) -> Result<Vec<u8>> {
        evidence::receipt_bytes(&regular_path(&self.root, path)?)
    }

    pub(super) fn create_validation_plan(
        &self,
        selection: &ReleaseSelection,
    ) -> Result<(String, Plan, Context)> {
        let directory = format!(
            "target/release-plans/{}-{}.{}",
            selection.group,
            std::process::id(),
            SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos()
        );
        // Plans must be relative inputs for Shared's runner, including with an
        // external CARGO_TARGET_DIR. Receipts keep their existing selected target.
        for prefix in ["target", "target/release-plans"] {
            let route = self.root.join(prefix);
            if route.exists() {
                require(
                    route.is_dir() && !fs::symlink_metadata(&route)?.file_type().is_symlink(),
                    "redirected plan parent",
                )?;
            } else {
                fs::create_dir(&route)?;
            }
        }
        fs::create_dir(self.root.join(&directory))?;
        let path = format!("{directory}/plan.json");
        let (plan, context) = self.validation_selection(selection, &path)?;
        for input in &context.inputs {
            regular_path(&self.root, input)?;
        }
        self.exec
            .write_bytes(&self.root.join(&path), &canonical(&plan)?)?;
        self.exec.write_bytes(
            &self.root.join(format!("{path}.context.json")),
            &canonical(&context)?,
        )?;
        // Refuse redirected target routes before executing any selected target.
        regular_path(&self.root, &path)?;
        Ok((path, plan, context))
    }

    pub(super) fn collect_validation(
        &self,
        selection: &ReleaseSelection,
        path: &str,
        plan: Plan,
        context: Context,
    ) -> Result<GroupValidation> {
        let directory = self.root.join(format!("{path}.logs"));
        require(
            !fs::symlink_metadata(&directory)?.file_type().is_symlink(),
            "redirected runner directory",
        )?;
        // Nested validation keeps its own directories under the inherited log
        // root. Select the exact plan snapshot, never the first directory.
        let plan_bytes = canonical(&plan)?;
        let mut entries = Vec::new();
        for entry in fs::read_dir(&directory)? {
            let entry = entry?;
            if entry.file_type()?.is_dir()
                && fs::read(entry.path().join("parsed-inputs.json.plan.json"))
                    .is_ok_and(|bytes| bytes == plan_bytes)
            {
                entries.push(entry);
            }
        }
        require(entries.len() == 1, "missing or ambiguous validation run")?;
        let runner = entries[0]
            .path()
            .strip_prefix(&self.root)?
            .to_str()
            .ok_or("non-UTF8 runner path")?
            .to_owned();
        let mut result = GroupValidation {
            schema: 2,
            plan_path: path.to_owned(),
            plan,
            context,
            runner,
            proof: serde_json::Value::Null,
            files: BTreeMap::new(),
        };
        self.check_validation_records(selection, &mut result, true)?;
        Ok(result)
    }

    pub(super) fn admit_validation(
        &self,
        selection: &ReleaseSelection,
        saved: &GroupValidation,
    ) -> Result<()> {
        require(saved.schema == 2, "unsupported group-validation receipt")?;
        let (plan, context) = self.validation_selection(selection, &saved.plan_path)?;
        require(
            plan == saved.plan && context == saved.context,
            "saved validation plan/baseline changed",
        )?;
        let mut checked = GroupValidation {
            schema: saved.schema,
            plan_path: saved.plan_path.clone(),
            plan,
            context,
            runner: saved.runner.clone(),
            proof: saved.proof.clone(),
            files: saved.files.clone(),
        };
        self.check_validation_records(selection, &mut checked, false)
    }

    pub(super) fn check_validation_records(
        &self,
        selection: &ReleaseSelection,
        record: &mut GroupValidation,
        initial: bool,
    ) -> Result<()> {
        require(
            record.plan.source == selection.source
                && record.plan.group == selection.group
                && record
                    .runner
                    .starts_with(&format!("{}.logs/validation.", record.plan_path)),
            "wrong validation evidence owner",
        )?;
        let proof_path = format!("{}.proof.json", record.plan_path);
        if initial {
            let raw = self.output(
                "perl",
                &[
                    "vendor/toko-tooling/scripts/ci/validation-evidence.pl",
                    "verify",
                    self.root.to_str().ok_or("non-UTF8 source root")?,
                    &record.plan_path,
                    &selection.source,
                    &selection.group,
                    &record.runner,
                    &format!("{}.context.json", record.plan_path),
                ],
            )?;
            record.proof = serde_json::from_str(&raw)?;
            require(
                record.proof["schema"] == 1
                    && record.proof["kind"] == "group-validation-evidence"
                    && record.proof["source"] == selection.source
                    && record.proof["group"] == selection.group
                    && record.proof["run"] == record.runner,
                "wrong shared evidence identity",
            )?;
            self.exec
                .write_bytes(&self.root.join(&proof_path), &canonical(&record.proof)?)?;
            let retained: BTreeMap<String, String> =
                serde_json::from_value(record.proof["files"].clone())?;
            record.files = retained
                .into_iter()
                .map(|(path, hash)| (format!("{}/{path}", record.runner), hash))
                .collect();
            for (path, hash) in [
                (record.plan_path.clone(), "plan_sha256"),
                (
                    format!("{}.context.json", record.plan_path),
                    "context_sha256",
                ),
            ] {
                record.files.insert(
                    path,
                    record.proof[hash]
                        .as_str()
                        .ok_or("missing shared evidence binding")?
                        .to_owned(),
                );
            }
            record
                .files
                .insert(proof_path.clone(), digest(&canonical(&record.proof)?));
        }
        // Shared reobserves inputs on initial admission. Recovery can have the
        // explicitly qualified release-version projection in place, so keep its
        // original self-contained proof immutable and let the existing source,
        // compiler/configuration and lock projection admission own that change.
        require(
            self.plan_bytes(&proof_path)? == canonical(&record.proof)?,
            "changed shared proof",
        )?;
        for (path, hash) in &record.files {
            require(
                hash_file(&regular_path(&self.root, path)?, 64 * 1024 * 1024)?
                    .sha256
                    .to_string()
                    == *hash,
                "missing or changed shared validation evidence",
            )?;
        }
        Ok(())
    }
}
