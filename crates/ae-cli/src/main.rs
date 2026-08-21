use clap::Parser;

/// ae — Agent-Eye: converts pixels into deterministic visual evidence.
#[derive(Parser, Debug)]
#[command(name = "ae", version, about)]
struct Cli {}

fn main() {
    let _cli = Cli::parse();
}
