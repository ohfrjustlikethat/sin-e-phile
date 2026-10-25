//! Full verification of a downloaded artefact — internal checksum AND compatibility,
//! not merely the 256-byte header peek that `state_of` does.
fn main() {
    let dir = std::path::PathBuf::from(std::env::args().nth(1).unwrap_or_else(|| "data".into()));
    for asset in sinephile_catalogue::assets::optional_assets(&dir) {
        match sinephile_catalogue::assets::verify(&asset) {
            Ok(()) => println!("  {:<16} VERIFIED", asset.name),
            Err(e) => println!("  {:<16} FAILED — {e}", asset.name),
        }
    }
}
