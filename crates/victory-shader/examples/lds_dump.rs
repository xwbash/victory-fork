//! Decodes a dumped console shader and prints every LDS access with its
//! address computation, to see whether each console lane touches only its
//! own dwords (the address derives from the lane number).
//! `cargo run --example lds_dump -- <file.cdxil>`

fn main() {
    let path = std::env::args().nth(1).expect("usage: lds_dump <file.cdxil>");
    let bytes = std::fs::read(&path).expect("read");
    let c = victory_shader::Container::parse(&bytes).expect("container");
    let dxil = c.part(victory_shader::FourCC(*b"DXIL")).expect("no DXIL part");
    let programs = victory_shader::native_programs(dxil.data).expect("programs");
    for p in &programs {
        println!(
            "program stage={:?} wave64={} code={}-bytes",
            p.stage,
            p.wave64,
            p.code.len()
        );
        let code = victory_shader::gfx10::decode(&p.code).expect("decode");
        println!("--- первые 14 инструкций программы:");
        for (i, inst) in code.iter().take(14).enumerate() {
            println!(
                "[{i:5}] {:?} {:?}",
                inst.name().unwrap_or("?"),
                inst.fields(p.wave64)
            );
        }
        let mut shown = 0usize;
        for (i, inst) in code.iter().enumerate() {
            let name = inst.name().unwrap_or("");
            if name.starts_with("ds_") && !name.contains("swizzle") && !name.contains("permute") {
                println!("[{i:5}] {name:?} fields={:?}", inst.fields(p.wave64));
                // the 12 instructions before it: the address computation
                for (j, prev) in code[i.saturating_sub(12)..i].iter().enumerate() {
                    println!(
                        "        {:5}: {:?} {:?}",
                        i.saturating_sub(12) + j,
                        prev.name().unwrap_or("?"),
                        prev.fields(p.wave64)
                    );
                }
                shown += 1;
                if shown > 8 {
                    println!("  ...(more)");
                    break;
                }
            }
        }
    }
}
