//! End-to-end check of subtask 5.9's download path, into a directory of your choosing so
//! a working artefact is never at risk:
//!
//!     cargo run -p sinephile-catalogue --example fetch_assets -- <dir>

#[tokio::main]
async fn main() {
    let dir = std::env::args().nth(1).unwrap_or_else(|| "data".into());
    let dir = std::path::PathBuf::from(dir);
    let assets = sinephile_catalogue::assets::optional_assets(&dir);

    println!(
        "  plan: {} files, {:.0} MB total",
        assets.len(),
        sinephile_catalogue::assets::total_bytes(&assets) as f64 / 1_048_576.0
    );
    for asset in &assets {
        println!(
            "    {:<16} {:>7.1} MB  {:?}",
            asset.name,
            asset.bytes as f64 / 1_048_576.0,
            sinephile_catalogue::assets::state_of(asset)
        );
    }

    for asset in &assets {
        if matches!(
            sinephile_catalogue::assets::state_of(asset),
            sinephile_catalogue::assets::AssetState::Present
        ) {
            continue;
        }
        println!("\n  fetching {}", asset.name);
        let started = std::time::Instant::now();
        let mut last = 0u64;
        match sinephile_catalogue::assets::download(asset, |p| {
            if p.downloaded - last > 50_000_000 {
                last = p.downloaded;
                println!("    {:.0} MB", p.downloaded as f64 / 1_048_576.0);
            }
        })
        .await
        {
            Ok(()) => println!(
                "    verified in {:.0}s — {:?}",
                started.elapsed().as_secs_f64(),
                sinephile_catalogue::assets::state_of(asset)
            ),
            Err(e) => {
                println!("    FAILED: {e}");
                std::process::exit(1);
            }
        }
    }
    println!("\n  done");
}
