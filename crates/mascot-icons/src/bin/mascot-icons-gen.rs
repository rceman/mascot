//! Dev-time generator: parses the Lucide SVG subset in `crates/mascot-icons/source/`
//! and rewrites `src/generated.rs`. Never part of the shipped library.
//!
//! Usage: `cargo run -p mascot-icons --bin mascot-icons-gen`

#[path = "../svgparse.rs"]
mod svgparse;

fn main() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    match svgparse::generate(mascot_icons::ICON_TABLE, &dir.join("source")) {
        Ok(content) => {
            let out = dir.join("src/generated.rs");
            if let Ok(existing) = std::fs::read_to_string(&out)
                && existing == content
            {
                println!("generated.rs already in sync");
                return;
            }
            std::fs::write(&out, content).expect("write generated.rs");
            println!("wrote {}", out.display());
        }
        Err(e) => {
            eprintln!("mascot-icons-gen: {e}");
            std::process::exit(1);
        }
    }
}
