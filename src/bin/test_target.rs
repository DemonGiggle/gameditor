use std::io::{self, BufRead, Write};

fn main() {
    let mut val_u8: u8 = 100;
    let mut val_u16: u16 = 1000;
    let mut val_u32: u32 = 100_000;
    let mut val_u64: u64 = 10_000_000;

    let stdin = io::stdin();
    let mut stdout = io::stdout();

    println!("=== Test Target for Memory Scanner ===");
    println!("Commands:");
    println!("  set u8 <value>");
    println!("  set u16 <value>");
    println!("  set u32 <value>");
    println!("  set u64 <value>");
    println!("  show");
    println!("  quit");
    println!();

    loop {
        println!(
            "u8  = {} (0x{:02X})\n\
             u16 = {} (0x{:04X})\n\
             u32 = {} (0x{:08X})\n\
             u64 = {} (0x{:016X})",
            val_u8, val_u8, val_u16, val_u16, val_u32, val_u32, val_u64, val_u64,
        );

        print!("> ");
        let _ = stdout.flush();

        let mut line = String::new();
        if stdin.lock().read_line(&mut line).unwrap_or(0) == 0 {
            break;
        }
        let parts: Vec<&str> = line.trim().split_whitespace().collect();

        match parts.as_slice() {
            ["quit" | "exit"] => break,
            ["show"] => continue,
            ["set", ty, val] => {
                match *ty {
                    "u8" => match val.parse::<u8>() {
                        Ok(v) => val_u8 = v,
                        Err(e) => println!("invalid u8: {e}"),
                    },
                    "u16" => match val.parse::<u16>() {
                        Ok(v) => val_u16 = v,
                        Err(e) => println!("invalid u16: {e}"),
                    },
                    "u32" => match val.parse::<u32>() {
                        Ok(v) => val_u32 = v,
                        Err(e) => println!("invalid u32: {e}"),
                    },
                    "u64" => match val.parse::<u64>() {
                        Ok(v) => val_u64 = v,
                        Err(e) => println!("invalid u64: {e}"),
                    },
                    other => println!("unknown type: {other} (use u8, u16, u32, u64)"),
                }
            }
            [] => continue,
            _ => println!("unknown command"),
        }
    }
}
