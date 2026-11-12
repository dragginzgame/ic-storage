//! Conservative, source-bound selection over the existing complete CI roster.
#[cfg_attr(
    not(test),
    expect(
        clippy::wildcard_imports,
        reason = "Private release submodules share the parent helper vocabulary."
    )
)]
use super::*;

pub(super) const POLICY: &str = "ci/release-validation.json";

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Policy {
    schema: u32,
    groups: Vec<String>,
    stages: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Stage {
    pub target: String,
    pub required: bool,
    pub reason: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Plan {
    pub schema: u32,
    pub source: String,
    pub group: String,
    pub scope: String,
    pub policy: String,
    pub inputs: Vec<String>,
    pub stages: Vec<Stage>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Context {
    pub schema: u32,
    pub source: String,
    pub group: String,
    pub baseline_tag: String,
    pub baseline: Option<String>,
    pub baseline_observation: serde_json::Value,
    pub changed: Vec<String>,
    pub inputs: Vec<String>,
    pub host: String,
    pub identities: [String; 3],
    pub configuration: BTreeMap<String, String>,
}

pub(super) fn canonical(value: &impl Serialize) -> Result<Vec<u8>> {
    // Value uses sorted map keys, matching jq -cS and Shared's canonical admission.
    let mut bytes = serde_json::to_vec(&serde_json::to_value(value)?)?;
    bytes.push(b'\n');
    Ok(bytes)
}

pub(super) fn regular_path(root: &Path, path: &str) -> Result<PathBuf> {
    require(
        !path.is_empty() && !path.bytes().any(|b| b.is_ascii_control()),
        "unsafe validation path",
    )?;
    let mut route = root.to_owned();
    for component in Path::new(path).components() {
        require(
            matches!(component, std::path::Component::Normal(_)),
            "non-relative validation path",
        )?;
        route.push(component);
        require(
            !fs::symlink_metadata(&route)?.file_type().is_symlink(),
            "redirected validation path",
        )?;
    }
    require(route.is_file(), "missing regular validation input")?;
    Ok(route)
}

// Only private adapter implementations and tests qualify. Public entrypoints,
// manifests, examples, shared tooling, locks and unclassified inputs stay full.
pub(super) fn adapter_local(group: &str, path: &str) -> bool {
    let prefix = format!("crates/ic-{group}/src/");
    let Some(relative) = path.strip_prefix(&prefix) else {
        return path.starts_with(&format!("releases/{group}/"))
            && Path::new(path).extension().is_some_and(|ext| ext == "md")
            || path == "docs/status/current.md";
    };
    Path::new(relative)
        .extension()
        .is_some_and(|ext| ext == "rs")
        && (relative == "tests.rs"
            || relative.starts_with("tests/")
            || group == "metrics-storage"
                && (relative.starts_with("codec/") || relative.starts_with("metric/"))
            || group == "logging-storage" && relative == "codec/mod.rs")
}

pub(super) fn stages(
    group: &str,
    changed: &[String],
    baseline: bool,
    roster: &[String],
) -> Vec<Stage> {
    let narrow = baseline
        && group != "storage"
        && !changed.is_empty()
        && changed.iter().all(|path| adapter_local(group, path));
    roster.iter().map(String::as_str).map(|target| {
        let reason = if narrow {
            match target {
                "validate-tooling" => Some("Private adapter code cannot affect the standalone repository helper, hooks or release scripts; their full source, recipes, dependencies and configuration are unchanged from the admitted comparison baseline."),
                "validate-storage-tests" | "validate-wasm-size" => Some("Core Storage source, tests, examples, manifests and locks are unchanged; Storage has no dependency on either adapter. Selected-adapter lint, features, docs and Wasm plus all-archive integration still run."),
                "validate-metrics-tests" if group == "logging-storage" => Some("Metrics Storage and its complete dependency inputs are unchanged and it does not consume Logging Storage; all-archive integration still runs."),
                "validate-logging-tests" if group == "metrics-storage" => Some("Logging Storage and its complete dependency inputs are unchanged and it does not consume Metrics Storage; all-archive integration still runs."),
                _ => None,
            }
        } else { None };
        Stage { target: target.to_owned(), required: reason.is_none(), reason: reason.unwrap_or("Applicable product, integration or policy obligation; shared, public, core or uncertain changes retain the complete roster.").to_owned() }
    }).collect()
}

// Union all locked versions/features rather than resolving a narrower graph.
// Any direct or transitive reverse edge invalidates the adapter-only omission.
pub(super) fn independent_adapter(lock: &str, group: &str) -> Result<bool> {
    let value: toml::Value = toml::from_str(lock)?;
    let packages = value
        .get("package")
        .and_then(toml::Value::as_array)
        .ok_or("missing locked package graph")?;
    let mut graph = BTreeMap::<String, Vec<String>>::new();
    for package in packages {
        let name = package
            .get("name")
            .and_then(toml::Value::as_str)
            .ok_or("missing locked package name")?;
        let edges = graph.entry(name.to_owned()).or_default();
        if let Some(dependencies) = package.get("dependencies") {
            for dependency in dependencies
                .as_array()
                .ok_or("invalid locked dependencies")?
            {
                edges.push(
                    dependency
                        .as_str()
                        .and_then(|s| s.split_whitespace().next())
                        .ok_or("invalid locked edge")?
                        .to_owned(),
                );
            }
        }
    }
    let selected = format!("ic-{group}");
    if !graph.contains_key(&selected) {
        return Ok(false);
    }
    let peer = if group == "metrics-storage" {
        "ic-logging-storage"
    } else {
        "ic-metrics-storage"
    };
    let mut pending = vec!["ic-storage".to_owned(), peer.to_owned()];
    // Additional workspace/path consumers cannot silently fall outside the
    // original three-group mapping after a manifest change has been released.
    pending.extend(
        packages
            .iter()
            .filter(|package| package.get("source").is_none())
            .filter_map(|package| package.get("name").and_then(toml::Value::as_str))
            .filter(|name| *name != selected)
            .map(str::to_owned),
    );
    let mut seen = std::collections::BTreeSet::new();
    while let Some(name) = pending.pop() {
        if name == selected {
            return Ok(false);
        }
        if seen.insert(name.clone()) {
            pending.extend(graph.get(&name).into_iter().flatten().cloned());
        }
    }
    Ok(true)
}

impl<E: Execute> Repository<E> {
    pub(super) fn admit_validation_policy(&self, source: &str, group: &str) -> Result<Vec<String>> {
        let raw = self.text(POLICY, Some(source))?;
        let policy: Policy = serde_json::from_str(&raw)?;
        let release_groups = self.release_groups(Some(source))?;
        for group in &release_groups {
            let package = format!("ic-{}", group.name);
            require(
                group.primary == package && group.members == [package],
                "group package ownership changed; requalify validation selection",
            )?;
        }
        let mut groups: Vec<_> = release_groups.into_iter().map(|g| g.name).collect();
        groups.sort();
        require(
            policy.schema == 1
                && canonical(&policy)? == raw.as_bytes()
                && policy.groups == groups
                && groups.iter().any(|name| name == group)
                && !policy.stages.is_empty()
                && policy.stages.iter().all(|stage| {
                    stage.starts_with("validate-")
                        && stage
                            .bytes()
                            .all(|b| b.is_ascii_alphanumeric() || b == b'-')
                })
                && policy
                    .stages
                    .iter()
                    .collect::<std::collections::BTreeSet<_>>()
                    .len()
                    == policy.stages.len(),
            "validation policy/group roster drift",
        )?;
        Ok(policy.stages)
    }

    pub(super) fn validation_selection(
        &self,
        selection: &ReleaseSelection,
        plan_path: &str,
    ) -> Result<(Plan, Context)> {
        let roster = self.admit_validation_policy(&selection.source, &selection.group)?;
        let baseline_tag = format!("{}-v{}", selection.group, selection.previous);
        let observation: serde_json::Value = serde_json::from_str(&self.output(
            "perl",
            &[
                "scripts/ci/validation-evidence.pl",
                "baseline",
                self.root.to_str().ok_or("non-UTF8 source root")?,
                &selection.source,
                &format!("refs/tags/{baseline_tag}"),
            ],
        )?)?;
        require(
            observation["schema"] == 1
                && observation["source"] == selection.source
                && observation["reference"] == format!("refs/tags/{baseline_tag}"),
            "wrong baseline observation",
        )?;
        let baseline = if observation["available"]
            .as_bool()
            .ok_or("missing baseline availability")?
        {
            Some(
                observation["baseline"]
                    .as_str()
                    .ok_or("missing observed baseline")?
                    .to_owned(),
            )
        } else {
            None
        };
        let changed = if let Some(base) = &baseline {
            let primary = self.release_group(Some(base))?.primary;
            require(
                toml_string(
                    &self.text(&package_manifest(&primary), Some(base))?,
                    "package",
                    "version",
                )? == selection.previous,
                "validation baseline version differs from release intent",
            )?;
            serde_json::from_value(observation["changes"].clone())?
        } else {
            Vec::new()
        };
        let inventory = self.output(
            "git",
            &["ls-tree", "-r", "--name-only", "-z", &selection.source],
        )?;
        let mut inputs: Vec<String> = inventory
            .split('\0')
            .filter(|path| !path.is_empty())
            .map(str::to_owned)
            .collect();
        require(
            inputs.contains(&POLICY.to_owned()) && inputs.contains(&"Makefile".to_owned()),
            "incomplete validation inventory",
        )?;
        let context = Context {
            schema: 1,
            source: selection.source.clone(),
            group: selection.group.clone(),
            baseline_tag,
            baseline,
            baseline_observation: observation,
            changed,
            inputs: inputs.clone(),
            host: self.output("uname", &["-sm"])?,
            identities: self.identities()?,
            configuration: self.configuration()?,
        };
        let stages = stages(
            &selection.group,
            &context.changed,
            context.baseline.is_some()
                && independent_adapter(
                    &self.text("Cargo.lock", Some(&selection.source))?,
                    &selection.group,
                )?,
            &roster,
        );
        inputs.push(format!("{plan_path}.context.json"));
        inputs.sort();
        Ok((
            Plan {
                schema: 1,
                source: selection.source.clone(),
                group: selection.group.clone(),
                scope: if stages.iter().all(|s| s.required) {
                    "workspace"
                } else {
                    "group"
                }
                .to_owned(),
                policy: POLICY.to_owned(),
                inputs,
                stages,
            },
            context,
        ))
    }
}
