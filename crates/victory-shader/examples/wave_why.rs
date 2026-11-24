//! Translates dumped console shaders offline and reports the wave size
//! each ended up with, with `VICTORY_WAVE_WHY=1` printing why a program
//! keeps 64-lane waves.
//! `cargo run --example wave_why -- <dir with .cdxil files> [hashes...]`

fn psv_wave(d: &[u8]) -> Option<(u32, u32)> {
    let n = u32_le(d.get(28..32)?) as usize;
    eprintln!("parts: {n}");
    for i in 0..n {
        let (off, sz) = (
            u32_le(d.get(32 + i * 8..36 + i * 8)?),
            u32_le(d.get(36 + i * 8..40 + i * 8)?),
        );
        eprintln!("  [{i}] off={off} size={sz} fourcc={:?}", d.get(off as usize..(off+4) as usize).map(|b| String::from_utf8_lossy(b).into_owned()));
        if d.get(off as usize..(off + 4) as usize)? == b"PSV0" && sz >= 32 {
            let p = &d[off as usize..(off + sz) as usize];
            let min = u32_le(p.get(20..24)?);
            let max = u32_le(p.get(24..28)?);
            return Some((min, max));
        }
    }
    None
}

fn u32_le(b: &[u8]) -> u32 {
    u32::from_le_bytes(b.try_into().unwrap())
}

fn main() {
    victory_shader::emit::set_host_max_wave(32);
    let mut args = std::env::args().skip(1);
    let dir = args.next().expect("usage: wave_why <dir> [hashes...]");
    let hashes: Vec<String> = args.collect();
    let entries: Vec<_> = std::fs::read_dir(&dir)
        .expect("dir")
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| {
            p.extension().is_some_and(|x| x == "cdxil")
                && p.file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|n| n.ends_with(".compute.cdxil"))
        })
        .filter(|p| {
            hashes.is_empty()
                || hashes.iter().any(|h| {
                    p.file_name()
                        .and_then(|n| n.to_str())
                        .is_some_and(|n| n.starts_with(h))
                })
        })
        .collect();
    for path in entries {
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("?").to_owned();
        let bytes = std::fs::read(&path).expect("read");
        eprintln!("=== {name}");
        match victory_shader::emit::translate(&bytes) {
            Ok(host) => {
                // the wave size sits in the PSV0 part: info[16..20] min,
                // info[20..24] max, after the u32 version field
                let ws = psv_wave(&host.dxil)
                    .map(|(min, max)| format!("min={min} max={max}"))
                    .unwrap_or_else(|| "no PSV0".into());
                let out = std::path::Path::new(&dir).join(format!("{name}.probe.dxil"));
                let _ = std::fs::write(&out, &host.dxil);
                println!("{name}: {ws} ok -> {}", out.display());
            }
            Err(e) => println!("{name}: translate failed: {e}"),
        }
    }
}
