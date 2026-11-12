use clap::Parser;

fn main() -> sloppytoppy::Result<()> {
    sloppytoppy::run(&sloppytoppy::Cli::parse())
}
