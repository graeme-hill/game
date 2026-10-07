//! Add editable locomotion clips to an existing workspace (explicit directory only).
fn main() {
    let mut args = std::env::args_os().skip(1);
    let directory = args
        .next()
        .expect("usage: animate_workspace DIR [--replace-locomotion]");
    let replace = match args.next() {
        None => false,
        Some(flag) if flag == "--replace-locomotion" => true,
        _ => panic!("usage: animate_workspace DIR [--replace-locomotion]"),
    };
    assert!(args.next().is_none(), "unexpected arguments");
    let directory = std::path::Path::new(&directory);
    let mut library = game::storage::load_workspace(directory).expect("load workspace");
    for character in &mut library.characters {
        let body = library
            .bodies
            .iter()
            .find(|body| body.id == character.body)
            .unwrap();
        for clip in game::animation::locomotion_clips(body) {
            if let Some(existing) = character
                .animations
                .iter_mut()
                .find(|existing| existing.name.eq_ignore_ascii_case(&clip.name))
            {
                if replace {
                    *existing = clip;
                }
            } else {
                character.animations.push(clip);
            }
        }
    }
    game::storage::save_workspace(directory, &library).expect("save animations");
}
