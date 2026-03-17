/// A memory address candidate from a scan.
#[derive(Clone, Debug)]
pub struct Candidate {
    pub address: u64,
    /// Byte width: 1, 2, 4, or 8.
    pub width: u8,
    /// Raw little-endian bytes of the found value.
    pub value: Vec<u8>,
    pub pinned: bool,
}

/// A frozen/pinned memory entry.
#[derive(Clone, Debug)]
pub struct Pin {
    pub id: u64,
    pub address: u64,
    pub width: u8,
    /// Value to repeatedly write.
    pub value: Vec<u8>,
    pub enabled: bool,
}

/// Commands sent from the UI thread to the worker thread.
#[derive(Debug)]
pub enum WorkerCmd {
    /// Attach to a process by PID.
    Attach(u32),
    /// Full memory scan for the given little-endian byte pattern.
    Scan(Vec<u8>),
    /// Re-scan existing candidates for a new value.
    Rescan(Vec<u8>),
    /// Write bytes to an address.
    Write { address: u64, value: Vec<u8> },
    /// Add a pin to the freeze list.
    PinAdd(Pin),
    /// Remove a pin by ID.
    PinRemove(u64),
    /// Toggle a pin's enabled state by ID.
    PinToggle(u64),
}

/// Results sent back from the worker thread to the UI.
#[derive(Debug)]
pub enum WorkerResult {
    Attached(u32),
    AttachFailed(String),
    ScanComplete(Vec<Candidate>),
    ScanError(String),
    WriteOk,
    WriteErr(String),
}
