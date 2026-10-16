//! Visual design tokens: one dark palette, type scale and fonts.
use super::super::DocKind;
use bevy::prelude::*;

#[derive(Clone, Copy)]
pub struct Palette {
    pub bg: Color,
    pub panel: Color,
    pub panel_header: Color,
    pub raised: Color,
    pub field: Color,
    pub field_hover: Color,
    pub hover: Color,
    pub press: Color,
    pub border: Color,
    pub divider: Color,
    pub text: Color,
    pub text_dim: Color,
    pub text_muted: Color,
    pub accent: Color,
    pub accent_hover: Color,
    pub accent_dim: Color,
    pub accent_text: Color,
    pub success: Color,
    pub warning: Color,
    pub danger: Color,
    pub danger_dim: Color,
    pub axis_x: Color,
    pub axis_y: Color,
    pub axis_z: Color,
    pub viewport: Color,
    pub grid: Color,
    pub grid_major: Color,
    pub socket: Color,
    pub socket_required: Color,
    pub shadow: Color,
    pub scrim: Color,
}

const fn hex(rgb: u32) -> Color {
    Color::srgb(
        ((rgb >> 16) & 0xff) as f32 / 255.,
        ((rgb >> 8) & 0xff) as f32 / 255.,
        (rgb & 0xff) as f32 / 255.,
    )
}

impl Palette {
    pub const fn get() -> Self {
        Self {
            bg: hex(0x111317),
            panel: hex(0x1a1d23),
            panel_header: hex(0x16191e),
            raised: hex(0x23272f),
            field: hex(0x14171b),
            field_hover: hex(0x1b1f25),
            hover: hex(0x272b34),
            press: hex(0x2f3440),
            border: hex(0x2b3039),
            divider: hex(0x0b0c0f),
            text: hex(0xe3e6eb),
            text_dim: hex(0xa3a9b4),
            text_muted: hex(0x6c7380),
            accent: hex(0x3d8bfd),
            accent_hover: hex(0x5a9dff),
            accent_dim: hex(0x1f3354),
            accent_text: hex(0x9cc4ff),
            success: hex(0x3fb37f),
            warning: hex(0xe8a23a),
            danger: hex(0xef5a5a),
            danger_dim: hex(0x3d1f22),
            axis_x: hex(0xe5534b),
            axis_y: hex(0x6cc04a),
            axis_z: hex(0x4c8dff),
            viewport: hex(0x2a2e35),
            grid: Color::srgba(1., 1., 1., 0.06),
            grid_major: Color::srgba(1., 1., 1., 0.13),
            socket: hex(0x3fd0e0),
            socket_required: hex(0xff7ab6),
            shadow: Color::srgba(0., 0., 0., 0.55),
            scrim: Color::srgba(0.03, 0.035, 0.045, 0.72),
        }
    }
    pub fn kind(&self, kind: DocKind) -> Color {
        match kind {
            DocKind::Body => hex(0xc387f2),
            DocKind::Prop => hex(0xf2b33d),
            DocKind::Character => hex(0x4fd18b),
            DocKind::Tile => hex(0x36c2d0),
            DocKind::World => hex(0x5b9bff),
            DocKind::Rules => hex(0xf27a9b),
        }
    }
    pub fn axis(&self, axis: usize) -> Color {
        [self.axis_x, self.axis_y, self.axis_z][axis]
    }
}

pub fn kind_glyph(kind: DocKind) -> &'static str {
    match kind {
        DocKind::Body => "◉",
        DocKind::Prop => "◆",
        DocKind::Character => "★",
        DocKind::Tile => "⬢",
        DocKind::World => "◎",
        DocKind::Rules => "⇄",
    }
}

/// Type scale (logical pixels).
pub mod size {
    pub const XS: f32 = 10.5;
    pub const SM: f32 = 11.5;
    pub const MD: f32 = 12.5;
    pub const LG: f32 = 14.;
    pub const XL: f32 = 18.;
    pub const HERO: f32 = 28.;
}

/// Layout metrics (logical pixels).
pub mod metric {
    pub const TOP_BAR: f32 = 40.;
    pub const STATUS_BAR: f32 = 24.;
    pub const LEFT_PANEL: f32 = 264.;
    pub const RIGHT_PANEL: f32 = 320.;
    pub const TAB_BAR: f32 = 34.;
    pub const TIMELINE: f32 = 150.;
    pub const ROW: f32 = 24.;
    pub const FIELD: f32 = 24.;
    pub const LABEL: f32 = 80.;
    pub const RADIUS: f32 = 5.;
}

#[derive(Resource, Clone)]
pub struct Fonts {
    pub regular: Handle<Font>,
    pub medium: Handle<Font>,
    pub semibold: Handle<Font>,
    pub mono: Handle<Font>,
    pub icons: Handle<Font>,
}

pub fn load_fonts(mut commands: Commands, assets: Res<AssetServer>) {
    commands.insert_resource(Fonts {
        regular: assets.load("fonts/Inter-Regular.ttf"),
        medium: assets.load("fonts/Inter-Medium.ttf"),
        semibold: assets.load("fonts/Inter-SemiBold.ttf"),
        mono: assets.load("fonts/JetBrainsMono-Regular.ttf"),
        icons: assets.load("fonts/DejaVuSans.ttf"),
    });
}
