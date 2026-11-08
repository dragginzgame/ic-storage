//! Diagnose actual vector source ranges using frozen baseline/candidate Wasm.
use clap::Parser;
use ic_platform::{pocket_ic::PocketIcBuilder, principal::ExternalPrincipal};
use ic_storage_benchmarks::{
    read_probe::{self, Probe, Read},
    *,
};
use std::{fs, io::Write, path::PathBuf};

#[derive(Parser)]
struct Arguments {
    #[arg(long)]
    baseline_wasm: PathBuf,
    #[arg(long)]
    candidate_wasm: PathBuf,
    #[arg(long)]
    probe_wasm: PathBuf,
    #[arg(long)]
    output: PathBuf,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Arguments::parse();
    let pic = PocketIcBuilder::new()
        .with_server_url(std::env::var("IC_TESTKIT_POCKET_IC_URL")?.parse()?)
        .with_application_subnet()
        .build()?;
    let probe_wasm = fs::read(args.probe_wasm)?;
    fs::create_dir(&args.output)?;
    let mut raw = fs::File::create(args.output.join("samples.jsonl"))?;
    let mut csv = fs::File::create(args.output.join("reads.csv"))?;
    writeln!(
        csv,
        "layout,rows,payload,index,combined,phase,source,length,source_mod_4096,source_mod_65536,regions_65536,destination,instructions"
    )?;
    for (layout, path, prefix) in [
        ("checksummed", args.baseline_wasm, 12),
        ("compact", args.candidate_wasm, 2),
    ] {
        let wasm = fs::read(path)?;
        for rows in [512, 1024, 2048] {
            for payload in [256, 2048] {
                let canister = pic.create_canister()?;
                pic.add_cycles(canister, 100_000_000_000_000)?;
                let init = Initialization {
                    case: Case::VectorSession,
                    cache_bytes: 0,
                    upstream_cache_slots: 0,
                    upstream_bucket_pages: 8.try_into()?,
                    cold_cache: false,
                    vector_open: false,
                };
                pic.install_canister(
                    canister,
                    wasm.clone(),
                    ic_wire::native::encode_request(0, &init, LIMITS)?,
                    None,
                )?;
                for start in (0..rows).step_by(128) {
                    let setup = Setup {
                        start: u64::from(start),
                        count: (rows - start).min(128),
                        payload,
                        ids: Vec::new(),
                    };
                    let reply = pic.update_call(
                        canister,
                        ExternalPrincipal::anonymous(),
                        "setup",
                        ic_wire::native::encode_request(SETUP_ID, &setup, LIMITS)?,
                    )?;
                    assert_eq!(
                        ic_wire::native::decode_response::<u64>(SETUP_ID, &reply, LIMITS)?,
                        u64::from(start + setup.count)
                    );
                }
                let image = pic.get_stable_memory(canister)?;
                // A passive diagnostic upgrade preserves the exact source bytes
                // populated by each original collection binary.
                pic.advance_time(std::time::Duration::from_secs(300))?;
                pic.upgrade_canister(canister, probe_wasm.clone(), Vec::new(), None)?;
                let mut indices = vec![0, rows - 2, rows - 1];
                if rows == 2048 {
                    // The compact layout aligns index 2015 at a source-page
                    // boundary. Include both neighbours and the reverse control.
                    indices.extend([2014, 2015, 2016]);
                }
                for index in indices {
                    let encoded =
                        SessionEnvelope::make(u64::from(index), payload, false).encode()?;
                    let matches: Vec<_> = image
                        .windows(encoded.len())
                        .enumerate()
                        .filter_map(|(i, bytes)| (bytes == encoded).then_some(i))
                        .collect();
                    assert_eq!(matches.len(), 1, "record must have one physical payload");
                    let offset = (matches[0] - prefix as usize) as u64;
                    let prefix_bytes = &image[offset as usize..offset as usize + prefix as usize];
                    let length = if prefix == 2 {
                        u32::from(u16::from_le_bytes(prefix_bytes.try_into()?))
                    } else {
                        u32::from_le_bytes(prefix_bytes[..4].try_into()?)
                    };
                    assert_eq!(length as usize, encoded.len());
                    for combined in [false, true] {
                        let request = Probe {
                            offset,
                            prefix,
                            payload: length,
                            combined,
                        };
                        let reply = pic.query_call(
                            canister,
                            ExternalPrincipal::anonymous(),
                            "probe",
                            ic_wire::native::encode_request(
                                read_probe::ID,
                                &request,
                                read_probe::LIMITS,
                            )?,
                        )?;
                        let reads: Vec<Read> = ic_wire::native::decode_response(
                            read_probe::ID,
                            &reply,
                            read_probe::LIMITS,
                        )?;
                        assert_eq!(reads.len(), if combined { 2 } else { 3 });
                        writeln!(
                            raw,
                            "{}",
                            serde_json::json!({"layout":layout,"rows":rows,"payload":payload,"index":index,"request":request,"reads":reads})
                        )?;
                        raw.flush()?;
                        for read in reads {
                            let start = usize::try_from(read.offset)?;
                            assert_eq!(read.bytes, image[start..start + read.bytes.len()]);
                            let regions = (read.offset + read.bytes.len() as u64 - 1) / 65536
                                - read.offset / 65536
                                + 1;
                            writeln!(
                                csv,
                                "{layout},{rows},{payload},{index},{combined},{},{},{},{},{},{regions},{},{}",
                                read.phase,
                                read.offset,
                                read.bytes.len(),
                                read.offset % 4096,
                                read.offset % 65536,
                                read.destination,
                                read.instructions
                            )?;
                        }
                    }
                }
                pic.stop_canister(canister, None)?;
                pic.delete_canister(canister, None)?;
            }
        }
    }
    Ok(())
}
