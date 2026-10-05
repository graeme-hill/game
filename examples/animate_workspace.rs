//! Add editable locomotion clips to an existing workspace (explicit directory only).
fn main() {
    let directory = std::env::args_os()
        .nth(1)
        .expect("usage: animate_workspace DIR");
    let directory = std::path::Path::new(&directory);
    let mut library = game::storage::load_workspace(directory).expect("load workspace");
    for character in &mut library.characters {
        let body = library
            .bodies
            .iter()
            .find(|body| body.id == character.body)
            .unwrap();
        for clip in game::animation::locomotion_clips(body) {
            if !character
                .animations
                .iter()
                .any(|existing| existing.name.eq_ignore_ascii_case(&clip.name))
            {
                character.animations.push(clip);
            }
        }
    }
    game::storage::save_workspace(directory, &library).expect("save animations");
}
