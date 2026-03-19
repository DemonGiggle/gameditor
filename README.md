# Game Slave

A Windows memory scanner with a graphical interface, built in Rust. Attach to a running process, scan its memory for values, narrow results with repeated scans, write new values, and freeze (pin) addresses so they stay at a desired value.

## Features

- **Process list** — browse and filter running Windows processes
- **Memory scanning** — initial scan for a value across all readable committed memory
- **Rescan / filter** — re-scan previous candidates to narrow results
- **Write** — overwrite a memory address with a new value
- **Freeze / pin** — continuously rewrite pinned addresses on a 200 ms loop
- **Byte widths** — supports 1, 2, 4, and 8-byte unsigned integers (little-endian)

## Project Structure

```
src/
  main.rs      — entry point, eframe window setup
  app.rs       — egui UI (process list, scan page, pin panel)
  worker.rs    — background thread handling scans, writes, and pins
  scanner.rs   — scan algorithms and value encoding
  memory.rs    — Windows API wrappers (VirtualQueryEx, Read/WriteProcessMemory)
  process.rs   — process enumeration via Toolhelp32
  types.rs     — shared data structures and message types
```

## Prerequisites

- [Rust](https://rustup.rs/) (stable toolchain)
- A Windows target. The scanner calls Win32 APIs and will only function on Windows. Non-Windows builds compile but return stub/no-op results.

### Cross-compiling from Linux

If you are building on Linux for a Windows target, add the MinGW cross-compilation toolchain and the corresponding Rust target:

```bash
# Ubuntu / Debian
sudo apt-get install gcc-mingw-w64-x86-64

# Add the Rust target
rustup target add x86_64-pc-windows-gnu
```

## Building a Windows Executable

### On Windows

```bash
cargo build --release
```

The executable will be at `target/release/game-slave.exe`.

### On Linux (cross-compile)

```bash
cargo build --release --target x86_64-pc-windows-gnu
```

The executable will be at `target/x86_64-pc-windows-gnu/release/game-slave.exe`.

## Running

```bash
# On Windows, after building:
.\target\release\game-slave.exe
```

The application opens a 1000x700 window. Select a process from the list, enter a value and byte width, then scan. Use rescan to filter results, write to change values, and pin to freeze them.

> **Note:** Scanning another process's memory typically requires administrator privileges.

## Running Tests

```bash
cargo test
```

Tests in `scanner.rs` and `worker.rs` cover value encoding, buffer search, scan logic, and worker command handling. They run on any platform (no Windows APIs required).

## License

See [LICENSE](LICENSE) if present, or contact the repository owner.
