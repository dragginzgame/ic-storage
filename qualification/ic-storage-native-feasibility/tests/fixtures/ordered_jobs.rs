//! Closed installed page transactions. Shared MigrationJob owns phase/epoch
//! policy; this adapter binds its trusted bytes to the exclusive native family.
use super::{
    BASE, MAGIC, checked,
    ordered_owner::ROOTS,
    ordered_runtime::{self, Stable},
    ordered_session,
};
use alloc::vec::Vec;
use core::sync::atomic::{AtomicU64, Ordering};
use ic_platform_runtime::raw as ic0;
use ic_storage::{
    checkpoint::{Memory, Pages, Record, Specification, Store},
    ordered::{Mutation, paged::PAGE_BYTES},
};
use ic_storage_native_feasibility::migration_job::{OrderedFamilyJob, ordered_family::Family};

pub(super) const SPECS: [Specification; 4] = [
    Specification {
        id: 40,
        bytes: 16 + 1024 * 1024,
    },
    Specification {
        id: 41,
        bytes: 16 + 1024,
    },
    Specification {
        id: 42,
        bytes: 16 + 1024,
    },
    Specification {
        id: 43,
        bytes: 24 + 64 * PAGE_BYTES as u64,
    },
];
static READ: AtomicU64 = AtomicU64::new(0);
static WRITTEN: AtomicU64 = AtomicU64::new(0);
static PAGE_READS: AtomicU64 = AtomicU64::new(0);
static LAST: [AtomicU64; 5] = [const { AtomicU64::new(0) }; 5];
#[derive(Clone, Copy)]
struct Meter;
impl Memory for Meter {
    fn size(&self) -> u64 {
        Stable.size()
    }
    fn grow(&self, pages: u64) -> Option<u64> {
        Stable.grow(pages)
    }
    fn read(&self, offset: u64, bytes: &mut [u8]) {
        READ.fetch_add(bytes.len() as u64, Ordering::Relaxed);
        if bytes.len() == PAGE_BYTES {
            PAGE_READS.fetch_add(1, Ordering::Relaxed);
        }
        Stable.read(offset, bytes);
    }
    fn write(&self, offset: u64, bytes: &[u8]) {
        WRITTEN.fetch_add(bytes.len() as u64, Ordering::Relaxed);
        Stable.write(offset, bytes);
    }
}
struct Backing {
    job: Record<Vec<u8>, Meter>,
    cursor: Record<Vec<u8>, Meter>,
    pages: Pages<PAGE_BYTES, Meter>,
}
impl Backing {
    fn open(create: bool) -> Self {
        let mut store = if create {
            Store::create(Meter, &SPECS)
        } else {
            Store::open(Meter, &SPECS)
        }
        .unwrap();
        if create {
            let _ = Record::create(store.take(40).unwrap(), Vec::<u8>::new()).unwrap();
        }
        let job = store.take(41).unwrap();
        let cursor = store.take(42).unwrap();
        let pages = store.take(43).unwrap();
        if create {
            Self {
                job: Record::create(
                    job,
                    OrderedFamilyJob::initial(BASE as u64)
                        .unwrap()
                        .encode()
                        .unwrap(),
                )
                .unwrap(),
                cursor: Record::create(cursor, Vec::new()).unwrap(),
                pages: Pages::create(pages).unwrap(),
            }
        } else {
            Self {
                job: Record::load(job, 1024).unwrap(),
                cursor: Record::load(cursor, 1024).unwrap(),
                pages: Pages::load(pages).unwrap(),
            }
        }
    }
    fn job(&self) -> OrderedFamilyJob {
        OrderedFamilyJob::decode(self.job.get()).unwrap()
    }
}
pub(super) fn initialize() {
    let _ = Backing::open(true);
}
fn other() -> u64 {
    if BASE == ordered_runtime::A {
        ordered_runtime::B as u64
    } else {
        ordered_runtime::A as u64
    }
}
pub(super) fn available() {
    assert_ne!(checked()[9], 2, "private recovery candidate");
}
fn enter() -> u64 {
    ordered_runtime::enter();
    for counter in [&READ, &WRITTEN, &PAGE_READS] {
        counter.store(0, Ordering::Relaxed);
    }
    ic0::instruction_counter()
}
fn finish(start: u64) {
    for (slot, value) in LAST.iter().zip([
        ic0::instruction_counter() - start,
        ordered_runtime::scratch_bytes(),
        READ.load(Ordering::Relaxed),
        WRITTEN.load(Ordering::Relaxed),
        PAGE_READS.load(Ordering::Relaxed),
    ]) {
        slot.store(value, Ordering::Relaxed);
    }
}
pub(super) fn resume() {
    let words = checked();
    if words[9] == 2 {
        let backing = Backing::open(false);
        let job = backing.job();
        let (family, _) = job.importing(BASE as u64, other(), words[8]).unwrap();
        assert_eq!(backing.pages.len(), family.page_count() as u64);
        job.reader(backing.cursor.get()).unwrap();
    }
}
// Only whole-heap replacement enters this path, including an explicit unfinished
// restart. No old prefix is borrowed or scanned. Compatible resumes use open().
pub(super) fn start(epoch: u64, restart: bool, fault: u64) {
    assert_eq!(ordered_runtime::header(), [0; 14], "replacement required");
    assert_eq!(ordered_runtime::auxiliary(), [[0; 9]; 3]);
    let mut backing = Backing::open(false);
    let mut job = backing.job();
    let family = if restart {
        job.restart_import(BASE as u64, other(), epoch)
    } else {
        job.begin_import(BASE as u64, other(), epoch)
    }
    .unwrap();
    assert_eq!(backing.pages.len(), family.page_count() as u64);
    let cursor = family.initial_cursor().unwrap();
    job.reader(&cursor).unwrap();
    ordered_runtime::publish(
        [MAGIC, BASE as u64, 0, 0, 0, 0, 0, 0, epoch, 2, 0, 0, 0, 1],
        false,
    );
    if fault == 4 {
        ordered_runtime::refuse_metadata();
    }
    ordered_session::open();
    assert_ne!(fault, 1);
    backing.cursor.set(cursor).unwrap();
    assert_ne!(fault, 2);
    backing.job.set(job.encode().unwrap()).unwrap();
    assert_ne!(fault, 3);
}

#[expect(
    unsafe_code,
    reason = "closed frozen family export into governed pages"
)]
#[unsafe(export_name = "canister_update job_seal")]
extern "C" fn seal() {
    let start = enter();
    let [epoch, target, fault] = super::args();
    assert!(fault <= 3);
    let mut words = checked();
    assert!(words[9] <= 1);
    let mut backing = Backing::open(false);
    let old = backing.job();
    backing.pages.clear();
    let manifests = ordered_session::with(|owner| {
        owner
            .read_all(|view| {
                ROOTS.map(|root| {
                    let offset = backing.pages.len();
                    view.export_pages(root.id, epoch, |index, bytes| {
                        let page: &[u8; PAGE_BYTES] = bytes.try_into().unwrap();
                        // The exporter visits descending indices. Reserve these unpublished
                        // slots first; only the final job seal authorizes any page contents.
                        while backing.pages.len() <= offset + index as u64 {
                            backing.pages.push(page).unwrap();
                        }
                        backing.pages.set(offset + index as u64, page).unwrap();
                        Ok::<_, ()>(())
                    })
                    .unwrap()
                })
            })
            .unwrap()
    });
    assert_ne!(fault, 1);
    let family = Family::new(manifests, epoch).unwrap();
    assert_eq!(backing.pages.len(), family.page_count() as u64);
    let next = old.seal(BASE as u64, target, epoch, &family).unwrap();
    backing.cursor.set(Vec::new()).unwrap();
    backing.job.set(next.encode().unwrap()).unwrap();
    assert_ne!(fault, 2);
    words[8] = epoch;
    words[9] = 1;
    ordered_runtime::publish(words, fault == 3);
    finish(start);
    ic0::msg_reply();
}
#[expect(
    unsafe_code,
    reason = "closed source cancellation under shared job transitions"
)]
#[unsafe(export_name = "canister_update job_cancel")]
extern "C" fn cancel() {
    let start = enter();
    let [epoch, target, fault] = super::args();
    assert!(fault <= 2);
    let mut words = checked();
    assert!(words[9] <= 1 && words[8] == epoch);
    let mut backing = Backing::open(false);
    let mut job = backing.job();
    job.cancel(BASE as u64, target, epoch).unwrap();
    backing.job.set(job.encode().unwrap()).unwrap();
    assert_ne!(fault, 1);
    words[9] = 0;
    ordered_runtime::publish(words, false);
    assert_ne!(fault, 2);
    finish(start);
    ic0::msg_reply();
}
#[expect(
    unsafe_code,
    reason = "one-page native/cursor/job publication in one IC transaction"
)]
#[unsafe(export_name = "canister_update job_step")]
extern "C" fn step() {
    let start = enter();
    let [epoch, index, fault] = super::args();
    assert!(fault <= 4);
    let mut words = checked();
    assert_eq!(words[8], epoch);
    let mut backing = Backing::open(false);
    let old = backing.job();
    let mut next = backing.job();
    let batch = next
        .batch(BASE as u64, other(), epoch, index.try_into().unwrap(), 1)
        .unwrap();
    assert_eq!(backing.pages.len(), batch.checkpoint.page_count() as u64);
    if batch.retry {
        // Exact latest retries acknowledge without replaying native writes or
        // changing trusted progress, including after application writes resume.
        assert_eq!(words[9], if batch.complete { 0 } else { 2 });
        old.reader(backing.cursor.get()).unwrap();
    } else {
        assert_eq!(words[9], 2);
        let mut reader = old.reader(backing.cursor.get()).unwrap();
        let mut cursor = backing.cursor.get().clone();
        let encoded = next.encode().unwrap();
        if let Some((root, local)) = batch.checkpoint.locate(batch.start as usize) {
            assert_eq!(reader.next_page(), Some(local));
            let page = backing.pages.get(index).unwrap();
            let records = reader.read_page(&page).unwrap();
            let mutations: Vec<_> = records
                .records()
                .map(|(key, value)| Mutation::Put(key, value))
                .collect();
            cursor = batch
                .checkpoint
                .next_cursor(batch.end as usize, &reader)
                .unwrap();
            // Prepare and validate fallible transient progress before mutations.
            next.reader(&cursor).unwrap();
            ordered_session::with(|owner| {
                owner
                    .compose(|mut view| view.apply(ROOTS[root].id, &mutations).unwrap())
                    .unwrap()
            });
        }
        assert_ne!(fault, 1);
        backing.cursor.set(cursor).unwrap();
        assert_ne!(fault, 2);
        backing.job.set(encoded).unwrap();
        assert_ne!(fault, 3);
        if batch.complete {
            words[9] = 0;
            ordered_runtime::publish(words, false);
        }
        let reply = super::encode_reply(batch.end as u64, fault == 4)
            .unwrap_or_else(|message| ic0::trap(message.as_bytes()));
        ic0::msg_reply_data_append(&reply);
    }
    finish(start);
    ic0::msg_reply();
}
#[expect(
    unsafe_code,
    reason = "query isolation of an attempted page transaction"
)]
#[unsafe(export_name = "canister_query query_job_step")]
extern "C" fn query_step() {
    step();
}
#[expect(
    unsafe_code,
    reason = "closed durable job diagnostic without native candidate access"
)]
#[unsafe(export_name = "canister_query job_status")]
extern "C" fn status() {
    ordered_runtime::enter();
    let backing = Backing::open(false);
    ic0::msg_reply_data_append(&backing.job().encode().unwrap());
    ic0::msg_reply();
}
#[expect(
    unsafe_code,
    reason = "closed page transaction cost and backing I/O evidence"
)]
#[unsafe(export_name = "canister_query job_metrics")]
extern "C" fn metrics() {
    for value in &LAST {
        ic0::msg_reply_data_append(&value.load(Ordering::Relaxed).to_le_bytes());
    }
    ic0::msg_reply();
}
#[expect(
    unsafe_code,
    reason = "closed corrupt-page injection for refusal and rollback evidence"
)]
#[unsafe(export_name = "canister_update job_damage")]
extern "C" fn damage() {
    ordered_runtime::enter();
    let [index, _, _] = super::args();
    let mut backing = Backing::open(false);
    let mut page = backing.pages.get(index).unwrap();
    page[100] ^= 1;
    backing.pages.set(index, &page).unwrap();
    ic0::msg_reply();
}
