//! SEELE CLI binary — full command surface lands in sprint-04.

fn main() -> anyhow::Result<()> {
    println!(
        "seele v{} — see `seele --help` once implemented",
        env!("CARGO_PKG_VERSION")
    );
    Ok(())
}
