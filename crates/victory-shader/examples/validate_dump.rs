//! Validates dumped translated shaders with `dxil.dll`: names the ones the
//! D3D12 runtime would refuse, with the validator's message.
//! `cargo run --example validate_dump -- <dir with .dxil files> [dxil.dll path]`

fn main() {
    let mut args = std::env::args().skip(1);
    let Some(dir) = args.next() else {
        eprintln!("usage: validate_dump <dir> [dxil.dll]");
        std::process::exit(2);
    };
    let dll = args.next().map(std::path::PathBuf::from);
    let mut validator = match victory_shader::emit::validator::Validator::load(dll.as_deref()) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("no validator: {e}");
            std::process::exit(2);
        }
    };
    let mut ok = 0usize;
    let mut bad = 0usize;
    let mut entries: Vec<_> = std::fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("read {dir}: {e}"))
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "dxil"))
        .collect();
    entries.sort();
    for path in entries {
        let Ok(mut bytes) = std::fs::read(&path) else { continue };
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("?").to_owned();
        match validator.validate(&mut bytes) {
            Ok(()) => ok += 1,
            Err(e) => {
                bad += 1;
                println!("BAD {name}: {e}");
            }
        }
    }
    println!("{ok} valid, {bad} invalid");
}
