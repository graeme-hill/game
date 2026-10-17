//! Create real editable voxel assets, or generate another world from an existing kit.
use game::{
    generation::{self, GenerateOptions},
    storage,
};
fn main() -> Result<(), String> {
    let mut args = std::env::args().skip(1).collect::<Vec<_>>();
    let replace = args.iter().any(|a| a == "--replace-kit");
    args.retain(|a| a != "--replace-kit");
    let mut args = args.into_iter();
    let dir = std::path::PathBuf::from(args.next().ok_or(
        "Usage: generate_neighbourhood WORKSPACE [--replace-kit] [SEED WIDTH DEPTH HOUSES PLANT_PERCENT]",
    )?);
    let mut library = storage::load_workspace(&dir)?;
    if replace {
        // Remove the old kit's tile and world sources, then install the current kit.
        for t in &library.tiles {
            storage::delete_resource(&dir, "tile", t.id)?;
        }
        for w in &library.worlds {
            storage::delete_resource(&dir, "world", w.id)?;
        }
        library.tiles.clear();
        library.worlds.clear();
        library.socket_rules = Default::default();
    }
    if library.tiles.is_empty() {
        generation::install(&mut library)?;
    } else {
        let values: Vec<_> = args.collect();
        let mut o = GenerateOptions {
            width: 7,
            depth: 7,
            ..Default::default()
        };
        for (n, v) in values.iter().enumerate() {
            let v = v.parse::<u64>().map_err(|e| e.to_string())?;
            match n {
                0 => o.seed = v,
                1 => o.width = v as usize,
                2 => o.depth = v as usize,
                3 => o.houses = v as usize,
                4 => o.plant_percent = u8::try_from(v).map_err(|e| e.to_string())?,
                _ => return Err("Too many arguments".into()),
            }
        }
        library.worlds.push(generation::generate(
            &library.tiles,
            &library.socket_rules,
            library.next_id(),
            &o,
        )?);
    }
    storage::save_workspace(&dir, &library)?;
    println!(
        "Saved {} voxel tiles and {} worlds in {}",
        library.tiles.len(),
        library.worlds.len(),
        dir.display()
    );
    Ok(())
}
