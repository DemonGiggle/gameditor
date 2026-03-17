pub struct MemRegion {
    pub base: u64,
    pub size: usize,
}

/// Enumerate all committed, readable (non-guard, non-noaccess) memory regions.
#[cfg(windows)]
pub fn query_readable_regions(handle_raw: usize) -> Vec<MemRegion> {
    use std::ffi::c_void;
    use windows::Win32::Foundation::HANDLE;
    use windows::Win32::System::Memory::{
        MEM_COMMIT, MEMORY_BASIC_INFORMATION, PAGE_GUARD, PAGE_NOACCESS, VirtualQueryEx,
    };

    let handle = HANDLE(handle_raw as *mut c_void);
    let mut regions = Vec::new();
    let mut address: usize = 0;

    loop {
        let mut mbi = MEMORY_BASIC_INFORMATION::default();
        let ret = unsafe {
            VirtualQueryEx(
                handle,
                Some(address as *const c_void),
                &mut mbi,
                std::mem::size_of::<MEMORY_BASIC_INFORMATION>(),
            )
        };
        if ret == 0 {
            break;
        }

        if mbi.State == MEM_COMMIT
            && mbi.Protect.0 != 0
            && mbi.Protect.0 & PAGE_NOACCESS.0 == 0
            && mbi.Protect.0 & PAGE_GUARD.0 == 0
        {
            regions.push(MemRegion {
                base: mbi.BaseAddress as u64,
                size: mbi.RegionSize,
            });
        }

        let next = mbi.BaseAddress as usize + mbi.RegionSize;
        if next <= address {
            break;
        }
        address = next;
    }
    regions
}

#[cfg(not(windows))]
pub fn query_readable_regions(_handle_raw: usize) -> Vec<MemRegion> {
    vec![]
}

/// Read `size` bytes from `address` in the target process.
/// Returns however many bytes were actually read (may be less than `size`).
#[cfg(windows)]
pub fn read_bytes(handle_raw: usize, address: u64, size: usize) -> Option<Vec<u8>> {
    use std::ffi::c_void;
    use windows::Win32::Foundation::HANDLE;
    use windows::Win32::System::Diagnostics::Debug::ReadProcessMemory;

    let handle = HANDLE(handle_raw as *mut c_void);
    let mut buf = vec![0u8; size];
    let mut read = 0usize;
    let ok = unsafe {
        ReadProcessMemory(
            handle,
            address as *const c_void,
            buf.as_mut_ptr() as *mut c_void,
            size,
            Some(&mut read),
        )
        .is_ok()
    };
    if !ok || read == 0 {
        return None;
    }
    buf.truncate(read);
    Some(buf)
}

#[cfg(not(windows))]
pub fn read_bytes(_handle_raw: usize, _address: u64, _size: usize) -> Option<Vec<u8>> {
    None
}

/// Write `data` to `address` in the target process.
#[cfg(windows)]
pub fn write_bytes(handle_raw: usize, address: u64, data: &[u8]) -> bool {
    use std::ffi::c_void;
    use windows::Win32::Foundation::HANDLE;
    use windows::Win32::System::Diagnostics::Debug::WriteProcessMemory;

    let handle = HANDLE(handle_raw as *mut c_void);
    let mut written = 0usize;
    unsafe {
        WriteProcessMemory(
            handle,
            address as *mut c_void,
            data.as_ptr() as *const c_void,
            data.len(),
            Some(&mut written),
        )
        .is_ok()
    }
}

#[cfg(not(windows))]
pub fn write_bytes(_handle_raw: usize, _address: u64, _data: &[u8]) -> bool {
    false
}
