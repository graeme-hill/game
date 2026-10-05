use crate::{editor, editor_animation, editor_scene, editor_ui};
use bevy::prelude::*;

pub struct EditorPlugin;

impl Plugin for EditorPlugin {
    fn build(&self, app: &mut App) {
        app.init_gizmo_group::<editor_scene::OverlayGuides>()
            .add_systems(Startup, (editor_scene::setup, editor_ui::load_pro_ui_font))
            .add_systems(
                Update,
                (
                    editor_scene::responsive_layout,
                    editor::controls,
                    editor::keyboard,
                    editor_scene::camera_input,
                    editor_ui::scroll_body_panes,
                    editor_scene::paint_viewport,
                    editor_scene::select_bone_viewport,
                    crate::editor_world::pick,
                    editor_scene::camera,
                    editor_scene::rebuild,
                    editor_ui::rebuild_ui,
                    editor_animation::preview,
                )
                    .chain(),
            )
            .add_systems(Update, (editor_scene::guides, crate::editor_world::guides))
            .add_systems(
                Update,
                editor_ui::apply_pro_ui_font.after(editor_ui::rebuild_ui),
            )
            .add_systems(
                PostUpdate,
                editor_ui::snapshot.after(bevy::ui::UiSystems::Layout),
            );
    }
}
