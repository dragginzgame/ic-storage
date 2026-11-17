use ic_storage::BootstrapAdmission;

fn prepare(admission: &BootstrapAdmission<'_>) {
    admission.open_memory(&ic_storage::StableKey::parse("app.journal.v1").unwrap());
}

fn main() {}
