// SPDX-FileCopyrightText: 2026 Kevin Carlson
// SPDX-License-Identifier: Apache-2.0
//! Round-trips one file and prints the OPF before and after.
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let src = std::env::args().nth(1).ok_or("usage: rt_one <epub>")?;
    let dest = std::env::temp_dir().join("f2-rt-one.epub");
    let o = futhark_f2::roundtrip::roundtrip(std::path::Path::new(&src), &dest);
    println!("outcome: {o:?}");
    let a = futhark_f2::archive::Archive::read(std::path::Path::new(&src))?;
    let b = futhark_f2::archive::Archive::read(&dest)?;
    for name in ["OEBPS/package.opf", "META-INF/container.xml"] {
        println!(
            "\n===== {name} BEFORE =====\n{}",
            String::from_utf8_lossy(&a.get(name).ok_or("missing")?.data)
        );
        println!(
            "===== {name} AFTER =====\n{}",
            String::from_utf8_lossy(&b.get(name).ok_or("missing")?.data)
        );
    }
    Ok(())
}
