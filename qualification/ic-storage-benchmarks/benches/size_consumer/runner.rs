//! Native validation and size accounting; never linked into either consumer Wasm.
use clap::Parser;
use ic_testkit::pocket_ic::{CanisterId, PocketIcBuilder, nonblocking::PocketIc};
use serde_json::json;
use std::{
    collections::{BTreeMap, BTreeSet},
    env, fs,
    io::Write,
    path::{Path, PathBuf},
};

#[derive(Parser)]
struct Arguments {
    /// Directory containing the matched consumer modules.
    output: PathBuf,
    /// Map/region mutations return presence rather than previous values.
    #[arg(long)]
    discard_mutations: bool,
}

const SELECTIONS: [&str; 8] = [
    "base",
    "map",
    "set",
    "vector",
    "cell",
    "region",
    "map-region",
    "all",
];
fn selected(selection: &str, index: usize) -> bool {
    selection == "all"
        || selection == ["map", "set", "vector", "cell", "region"][index]
        || selection == "map-region" && (index == 0 || index == 4)
}
fn filter(selection: &str, values: [u64; 5]) -> Vec<u64> {
    values
        .into_iter()
        .enumerate()
        .filter_map(|(i, v)| selected(selection, i).then_some(v))
        .collect()
}

fn mutation_result(selection: &str, mut values: [u64; 5], discard: bool) -> Vec<u64> {
    if discard {
        for index in [0, 4] {
            values[index] = u64::from(values[index] != u64::MAX);
        }
    }
    filter(selection, values)
}
#[derive(Default)]
struct Model {
    map: BTreeMap<u64, u64>,
    set: BTreeSet<u64>,
    vector: Vec<u64>,
    cell: u64,
}
impl Model {
    fn put(&mut self, key: u64, value: u64) -> [u64; 5] {
        let old = self.map.insert(key, value).unwrap_or(u64::MAX);
        let inserted = self.set.insert(key);
        self.vector.push(value);
        let cell = std::mem::replace(&mut self.cell, value);
        [
            old,
            u64::from(inserted),
            self.vector.len() as u64,
            cell,
            old,
        ]
    }
    fn get(&self, key: u64) -> [u64; 5] {
        let value = self.map.get(&key).copied().unwrap_or(u64::MAX);
        [
            value,
            u64::from(self.set.contains(&key)),
            self.vector.get(key as usize).copied().unwrap_or(u64::MAX),
            self.cell,
            value,
        ]
    }
    fn remove(&mut self, key: u64) -> [u64; 5] {
        let old = self.map.remove(&key).unwrap_or(u64::MAX);
        [
            old,
            u64::from(self.set.remove(&key)),
            self.vector.pop().unwrap_or(u64::MAX),
            std::mem::take(&mut self.cell),
            old,
        ]
    }
    fn scan(&self, start: u64) -> [u64; 5] {
        [
            self.map
                .range(start..)
                .take(16)
                .map(|(k, v)| k ^ v)
                .fold(0, u64::wrapping_add),
            self.set.iter().take(16).copied().fold(0, u64::wrapping_add),
            self.vector
                .iter()
                .take(16)
                .copied()
                .fold(0, u64::wrapping_add),
            self.cell,
            self.map
                .range(start..)
                .take(16)
                .map(|(&key, _)| key)
                .fold(0, u64::wrapping_add),
        ]
    }
}
fn decode(bytes: &[u8]) -> Vec<u64> {
    assert_eq!(bytes.len() % 8, 0);
    bytes
        .as_chunks::<8>()
        .0
        .iter()
        .map(|b| u64::from_le_bytes(*b))
        .collect()
}
async fn check_extrema(pic: &PocketIc, canister: CanisterId, selection: &str, model: &Model) {
    let expected = if selected(selection, 0) {
        let first = model
            .map
            .first_key_value()
            .map(|(&k, &v)| (k, v))
            .unwrap_or((u64::MAX, u64::MAX));
        let last = model
            .map
            .last_key_value()
            .map(|(&k, &v)| (k, v))
            .unwrap_or((u64::MAX, u64::MAX));
        vec![first.0, first.1, last.0, last.1]
    } else {
        Vec::new()
    };
    let response = pic
        .query_call(canister, canister, "extrema", vec![])
        .await
        .unwrap();
    assert_eq!(decode(&response), expected, "extrema {selection}");
}
async fn qualify(pic: &PocketIc, wasm: &[u8], selection: &str, discard: bool) -> usize {
    let canister = pic.create_canister().await;
    pic.add_cycles(canister, 100_000_000_000_000).await;
    pic.install_canister(canister, wasm.to_vec(), vec![], None)
        .await;
    let mut model = Model::default();
    check_extrema(pic, canister, selection, &model).await;
    let mut calls = 1;
    let mut put = async |key: u64, value: u64, model: &mut Model| {
        let arg = [key.to_le_bytes(), value.to_le_bytes()].concat();
        let response = pic
            .update_call(canister, canister, "put", arg)
            .await
            .unwrap();
        assert_eq!(
            decode(&response),
            mutation_result(selection, model.put(key, value), discard),
            "put {selection}"
        );
        calls += 1;
    };
    for key in (0..96).rev() {
        put(key, key * 11 + 3, &mut model).await;
    }
    for key in [0, 47, 95] {
        put(key, key * 17 + 5, &mut model).await;
    }
    for phase in 0..3 {
        check_extrema(pic, canister, selection, &model).await;
        calls += 1;
        for key in [0_u64, 47, 95, 999] {
            let response = pic
                .query_call(canister, canister, "get", key.to_le_bytes().to_vec())
                .await
                .unwrap();
            assert_eq!(
                decode(&response),
                filter(selection, model.get(key)),
                "get {selection} phase {phase}"
            );
            calls += 1;
        }
        for start in [0_u64, 17, 80, 999] {
            let response = pic
                .query_call(canister, canister, "scan", start.to_le_bytes().to_vec())
                .await
                .unwrap();
            assert_eq!(
                decode(&response),
                filter(selection, model.scan(start)),
                "scan {selection} phase {phase}"
            );
            calls += 1;
        }
        if phase == 1 {
            for key in [0_u64, 47, 95, 999] {
                let response = pic
                    .update_call(canister, canister, "remove", key.to_le_bytes().to_vec())
                    .await
                    .unwrap();
                assert_eq!(
                    decode(&response),
                    mutation_result(selection, model.remove(key), discard),
                    "remove {selection}"
                );
                calls += 1;
            }
        }
        if phase < 2 {
            pic.upgrade_canister(canister, wasm.to_vec(), vec![], None)
                .await
                .unwrap();
        }
    }
    calls
}
fn leb(bytes: &[u8], position: &mut usize) -> usize {
    let mut value = 0;
    let mut shift = 0;
    loop {
        let byte = bytes[*position];
        *position += 1;
        value |= usize::from(byte & 127) << shift;
        if byte < 128 {
            return value;
        }
        shift += 7;
        assert!(shift < usize::BITS);
    }
}
fn string<'a>(bytes: &'a [u8], position: &mut usize) -> &'a str {
    let size = leb(bytes, position);
    let start = *position;
    *position += size;
    std::str::from_utf8(&bytes[start..*position]).unwrap()
}
fn sizes(bytes: &[u8]) -> (usize, usize, usize, BTreeSet<String>) {
    assert_eq!(&bytes[..8], b"\0asm\x01\0\0\0");
    let (mut code, mut data, mut custom) = (0, 0, 0);
    let mut exports = BTreeSet::new();
    let mut pos = 8;
    while pos < bytes.len() {
        let start = pos;
        let id = bytes[pos];
        pos += 1;
        let payload = leb(bytes, &mut pos);
        let end = pos + payload;
        match id {
            0 => {
                let name = string(bytes, &mut pos);
                assert_ne!(name, "name", "size profile must strip function names");
                custom += end - start;
            }
            7 => {
                let count = leb(bytes, &mut pos);
                for _ in 0..count {
                    let name = string(bytes, &mut pos).to_owned();
                    pos += 1;
                    let _ = leb(bytes, &mut pos);
                    if name.starts_with("canister_") {
                        exports.insert(name);
                    }
                }
            }
            10 => code = payload,
            11 => data = payload,
            _ => {}
        }
        pos = end;
    }
    assert_eq!(pos, bytes.len());
    (code, data, custom, exports)
}
#[tokio::main(flavor = "current_thread")]
async fn main() {
    let arguments = Arguments::parse();
    let output = Path::new(&arguments.output);
    let url = env::var("IC_TESTKIT_POCKET_IC_URL").expect("prepared Testkit server URL");
    let pic = PocketIcBuilder::new()
        .with_max_request_time_ms(Some(300_000))
        .with_server_url(url.parse().unwrap())
        .with_application_subnet()
        .build_async()
        .await;
    let expected: BTreeSet<_> = [
        "canister_init",
        "canister_post_upgrade",
        "canister_update put",
        "canister_update remove",
        "canister_query get",
        "canister_query scan",
        "canister_query extrema",
    ]
    .map(str::to_owned)
    .into();
    let mut records = Vec::new();
    for selection in SELECTIONS {
        for backend in ["storage", "icss"] {
            let wasm = fs::read(output.join(format!("{selection}-{backend}.wasm"))).unwrap();
            let (code, data, custom, exports) = sizes(&wasm);
            assert_eq!(exports, expected, "export parity");
            let calls = qualify(&pic, &wasm, selection, arguments.discard_mutations).await;
            records.push(json!({"selection": selection, "backend": backend, "module_bytes": wasm.len(), "code_section_payload_bytes": code, "data_section_payload_bytes": data, "custom_section_bytes": custom, "validated_calls": calls, "upgrades": 2, "discard_mutations": arguments.discard_mutations}));
            println!(
                "Validated {selection}/{backend}: {} bytes, {calls} calls, 2 upgrades",
                wasm.len()
            );
        }
    }
    fs::write(
        output.join("results.json"),
        serde_json::to_vec_pretty(&records).unwrap(),
    )
    .unwrap();
    let mut csv = fs::File::create(output.join("sizes.csv")).unwrap();
    writeln!(
        csv,
        "selection,storage_bytes,icss_bytes,storage_minus_icss_bytes"
    )
    .unwrap();
    for pair in records.as_chunks::<2>().0 {
        let storage = pair[0]["module_bytes"].as_i64().unwrap();
        let icss = pair[1]["module_bytes"].as_i64().unwrap();
        writeln!(
            csv,
            "{},{storage},{icss},{}",
            pair[0]["selection"].as_str().unwrap(),
            storage - icss
        )
        .unwrap();
    }
    pic.drop().await;
}
