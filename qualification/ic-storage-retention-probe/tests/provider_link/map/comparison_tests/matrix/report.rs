use super::*;
use std::fmt::Write;

pub(super) fn render(
    samples: &[Sample],
    seeds: &[u64],
    profile: &str,
) -> Result<String, &'static str> {
    if samples.is_empty() || seeds.is_empty() {
        return Err("empty matrix");
    }
    let mut groups = BTreeMap::<(u64, u64, u64, usize), BTreeMap<(u64, bool), &Sample>>::new();
    for sample in samples {
        if !seeds.contains(&sample.seed)
            || sample.instructions == 0
            || sample.units == 0
            || sample.operation >= OPERATIONS.len()
        {
            return Err("invalid sample");
        }
        if groups
            .entry((sample.rows, sample.other, sample.batch, sample.operation))
            .or_default()
            .insert((sample.seed, sample.native), sample)
            .is_some()
        {
            return Err("duplicate sample");
        }
    }
    let mut text = format!(
        "# Storage comparison matrix\n\nProfile: {profile}. {} verified samples. Rust 1.100.0; ic-stable-structures 0.7.2.\n\nWhole-update IC instructions, including arguments and reply; meter epilogue and host RPC latency excluded. Values are medians across deterministic key schedules, not statistical confidence intervals. Ratio is native/stable; below 1 favours native. Control is not subtracted.\n\nNative uses the generation-owned service and trusted-source access. This is not the older per-message guarded fixture. Both backends operate on u64 keys/values. Full scans compare unordered sums, not ordered traversal semantics.\n\n| Rows | Sibling rows | Batch | Operation | Native median | Stable median | Native/stable |\n| ---: | ---: | ---: | --- | ---: | ---: | ---: |\n",
        samples.len()
    );
    for ((rows, other, batch, operation), values) in groups {
        if values.len() != seeds.len() * 2
            || seeds.iter().any(|seed| {
                !values.contains_key(&(*seed, false)) || !values.contains_key(&(*seed, true))
            })
        {
            return Err("incomplete pair");
        }
        let medians = [true, false].map(|native| {
            let mut values: Vec<_> = values
                .values()
                .filter(|s| s.native == native)
                .map(|s| s.instructions)
                .collect();
            values.sort_unstable();
            values[values.len() / 2]
        });
        writeln!(
            text,
            "| {rows} | {other} | {batch} | {} | {} | {} | {:.3} |",
            OPERATIONS[operation],
            medians[0],
            medians[1],
            medians[0] as f64 / medians[1] as f64
        )
        .unwrap();
    }
    text.push_str("\nMemory observations are reservations/endpoints, not live allocation peaks or total memory efficiency. Native slot bytes exclude its controller and transient space; stable structures uses default 128-page MemoryManager buckets. See matrix.csv for each sample.\n\n| Backend | Maximum Wasm memory bytes | Maximum stable memory bytes | Maximum native slot bytes |\n| --- | ---: | ---: | ---: |\n");
    for native in [true, false] {
        let memory: [u64; 3] = core::array::from_fn(|index| {
            samples
                .iter()
                .filter(|s| s.native == native)
                .map(|s| s.memory[index])
                .max()
                .unwrap()
        });
        writeln!(
            text,
            "| {} | {} | {} | {} |",
            if native {
                "native-session"
            } else {
                "stable-structures"
            },
            memory[0],
            memory[1],
            memory[2]
        )
        .unwrap();
    }
    text.push_str("\nSetup, upgrades, recovery, allocator peaks, large values, ordered ranges and populations beyond this fixture's bound are outside this matrix. A failed run has no MATRIX/COMPLETE marker; partial CSV rows are not a successful comparison.\n");
    Ok(text)
}
