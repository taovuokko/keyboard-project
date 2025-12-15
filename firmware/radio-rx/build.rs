use std::env;
use std::fs;
use std::path::PathBuf;

fn parse_hex_u32(val: &str) -> Option<u32> {
    let trimmed = val.trim();
    if let Some(stripped) = trimmed.strip_prefix("0x").or_else(|| trimmed.strip_prefix("0X")) {
        u32::from_str_radix(stripped, 16).ok()
    } else {
        trimmed.parse::<u32>().ok()
    }
}

fn main() {
    println!("cargo:rerun-if-env-changed=APP_BASE");
    println!("cargo:rerun-if-env-changed=FLASH_SIZE");
    println!("cargo:rerun-if-env-changed=RAM_BASE");
    println!("cargo:rerun-if-env-changed=RAM_SIZE");

    let app = env::var("APP_BASE")
        .ok()
        .and_then(|v| parse_hex_u32(&v))
        .expect("set APP_BASE (e.g. 0x26000 from INFO_UF2.TXT)");
    let flash_size = env::var("FLASH_SIZE")
        .ok()
        .and_then(|v| parse_hex_u32(&v))
        .expect("set FLASH_SIZE (from INFO_UF2.TXT)");
    if app >= flash_size {
        panic!("APP_BASE must be below FLASH_SIZE");
    }
    let ram_base = env::var("RAM_BASE")
        .ok()
        .and_then(|v| parse_hex_u32(&v))
        .unwrap_or(0x2000_0000);
    let ram_size = env::var("RAM_SIZE")
        .ok()
        .and_then(|v| parse_hex_u32(&v))
        .unwrap_or(0x0004_0000);

    let flash_len = flash_size - app;
    let memory_x = format!(
        "MEMORY {{\n  FLASH : ORIGIN = {:#010x}, LENGTH = {:#010x}\n  RAM : ORIGIN = {:#010x}, LENGTH = {:#010x}\n}}\n",
        app, flash_len, ram_base, ram_size
    );
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    fs::write(out_dir.join("memory.x"), &memory_x).unwrap();
    fs::write(PathBuf::from("memory.x"), &memory_x).unwrap();
    println!("cargo:rustc-link-search={}", out_dir.display());
}
