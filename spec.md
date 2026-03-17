# Spec: Windows Memory Scanner (Rust)

## 1. Scope

A Windows desktop application (Rust) that:

- Lists running processes
- Attaches to a selected process
- Scans memory for a value (by byte width)
- Filters results via repeated scans
- Writes values to memory
- Pins/freezes values

---

## 2. Tech Stack

- Language: Rust (stable)
- UI: egui/eframe (recommended)
- Windows API: `windows` crate

Required APIs:
- Process enum: Toolhelp32
- Memory: `OpenProcess`, `ReadProcessMemory`, `WriteProcessMemory`, `VirtualQueryEx`

---

## 3. Core Modules

### 3.1 UI
- Process list page
- Scan page
- Candidate table
- Pin list

### 3.2 Process Layer
- Enumerate processes
- Open process handle

### 3.3 Scan Engine
- Full memory scan
- Candidate storage
- Re-scan filtering

### 3.4 Freeze Engine
- Periodic write loop
- Pin management

---

## 4. Workflow

### Step 1: Select Process
- Show name + PID
- Attach to process

### Step 2: Initial Scan
Input:
- Value
- Byte width (1/2/4/8)

Action:
- Scan readable memory
- Record matching addresses

### Step 3: Re-scan
- Re-check only candidate addresses
- Keep matching values

### Step 4: Modify
- Write new value to selected address

### Step 5: Freeze
- Periodically rewrite pinned values

---

## 5. Data Types

Supported:

| Bytes | Type |
|------|------|
| 1 | u8 |
| 2 | u16 |
| 4 | u32 |
| 8 | u64 |

- Little-endian
- Exact match only (v1)

---

## 6. Memory Rules

Scan only:
- Committed
- Readable pages

Skip:
- No-access
- Guard pages

---

## 7. Data Structures

```rust
struct Candidate {
    address: u64,
    width: u8,
    value: Vec<u8>,
    pinned: bool,
}

struct Session {
    pid: u32,
    candidates: Vec<Candidate>,
}

struct Pin {
    address: u64,
    width: u8,
    value: Vec<u8>,
    enabled: bool,
}
```

---

## 8. UI Layout

### Process Page
- Table: name + PID
- Attach button

### Scan Page
- Value input
- Width selector
- [First Scan] [Next Scan]

### Result Table
- Address
- Value
- Width
- Pinned
- Actions: Write / Pin

---

## 9. Concurrency

- UI thread: rendering
- Worker thread:
  - scan
  - rescan
  - freeze loop

Requirement:
- No blocking UI

---

## 10. Freeze Engine

- Loop interval: 100–500 ms
- For each pinned address:
  - Write value

---

## 11. Error Handling

Handle:
- Access denied
- Process exit
- Invalid address
- Read/write failure

No crash allowed.

---

## 12. Milestones

1. Process list
2. Attach + memory scan
3. Re-scan filtering
4. Write memory
5. Freeze values

---

## 13. Acceptance

System works if:

- Can list processes
- Can attach
- Can scan by value + width
- Can filter results
- Can write value
- Can freeze value
