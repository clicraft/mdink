use std::fs;
use std::io::Write;

use clap::CommandFactory;
use clap_complete::Shell;
use flate2::write::GzEncoder;
use flate2::Compression;

#[path = "../../src/cli.rs"]
mod cli;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    match args.get(1).map(String::as_str) {
        Some("dist-assets") => dist_assets(),
        Some(other) => {
            eprintln!("unknown subcommand: {other}");
            eprintln!("usage: cargo xtask dist-assets");
            std::process::exit(1);
        }
        None => {
            eprintln!("usage: cargo xtask dist-assets");
            std::process::exit(1);
        }
    }
}

fn dist_assets() {
    let assets_dir = "assets";
    let completions_dir = "assets/completions";

    fs::create_dir_all(completions_dir).expect("failed to create assets/completions/");

    // ── Man page ───────────────────────────────────────────────────
    let cmd = cli::Cli::command();
    let mut man_buf = Vec::new();
    clap_mangen::Man::new(cmd).render(&mut man_buf).expect("failed to render man page");

    let gz_path = format!("{assets_dir}/mdink.1.gz");
    let gz_file = fs::File::create(&gz_path).expect("failed to create mdink.1.gz");
    let mut encoder = GzEncoder::new(gz_file, Compression::best());
    encoder.write_all(&man_buf).expect("failed to gzip man page");
    encoder.finish().expect("failed to finish gzip");
    println!("  {gz_path}");

    // ── Shell completions ──────────────────────────────────────────
    let mut cmd = cli::Cli::command();
    for shell in [Shell::Bash, Shell::Zsh, Shell::Fish] {
        let path =
            clap_complete::generate_to(shell, &mut cmd, "mdink", completions_dir)
                .expect("failed to generate completion");
        println!("  {}", path.display());
    }
}
