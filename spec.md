# game

## description

This "game" is more like a toy ala townscaper. The purpose is to design blocks
that can be used to generate a world, and then to create a character that can
run around in the world.

Players design their own character and world. That is the point.

## character creation

The game includes an ultra simplified character creator including building an
animating an armature. It is vastly simplified compared to what you would find
in blender.

The character's body is derived from the armature with parameters for thickess
of body and limbs based on parameters of those bones. You are essentially
building a stick man.

The skeleton and player model combine to make a body. The body also has mount
points for clothing, props, etc.

The body is not a regular mesh like props and landscape tiles, it is a volume
that gets rendered with raymarched signed distance fields so that they look like
extremely smooth pieces of viscous goo. You can only choose the colour of the
body parts and the spheres or capsules bound to each bone. This is similar to
what is already built in ~/googuys

## props and clothing

Users can model items like hats, shirts, swords etc. They have anchor points
defined. When making a character items can be attached by connecting their
anchor point to one of the mount points on the character's body.

For now the only modeling mode is voxel style with blocky rendering.

## voxel editor

props and items are modeled in a voxel editor. When you begin editing you are in
a screen where you see a 3d grid and you can place blocks of chosen colour.
There is a maximum resolution defined, and then you can switch modes to be in
1x, 2x, 4x, 8x, 16x, etc mode. If you are in 16x mode then when you place a
block you are actually placing a 16x16 block. The system should be built such
that if you choose to place a few 16x16 blocks then the overhead is low. You end
up with an optimized number of quads not a massive grid of tiny quads all
adjacent with matching colours.

## landscape design

### tiles

The landscape is built out of tiles. Tiles can have any shape. They are built in
a similar way to character props except that they are meant to anchor to other
tiles rather than to mount points on a character. They use the same modeling
techniques (ie: voxels for now with colours and not textures). Just like a prop
you can put anchor points on a tile. Anchor points also have an anchor type.

### tile connections

Users can define a many-to-many relationship between anchor point types to
control which connections are valid or not. Positions of anchor points have a
minimum resolution which equals the smallest voxel size. You cannot anchor one
tile to another if another unused anchor point on that tile would then be
unusable or would line of with an anchor point of another piece which is not a
valid connection based on the compatibility matrix.

### world generation

Based on the library of tiles available players can start building in a manual
mode where they place an initial tile, and then manually add compatible tiles at
its anchor points. eg: something like this:

- choose starting tile, it is placed at origin
- see on screen indicators for each of its anchor points
- click one of the anchor points and a list of valid tiles to connect pops up
- choose a valid connecting tile and that second tile appears, and soforth

At any point the player can choose to auto-generate the rest of the tiles and it
uses a wave function collapse style of algorithm to generate a valid map based
on allowed connections.

## playing

You can just spawn the character you designed in the world you also designed and
run around and look at it in third person perspective.

## rendering style

Everything is cartoon style cel shaded using forward rendering in a similar
style and technique used in Pokemon X and Y:

- There is a rendering pass that renders object IDs
- Future rendering pass uses object IDs to draw crisp, fixed pixel width black
  outlines around the OUTSIDE of objects (ie: proper OUTlines not inset lines).

## tools

The game is entirely source code defined and without a visual editor (because
the game is the editor). Purely written in rust with bevy game engine. Fully
embrace the ECS pattern and carefully design the game to have tidy, decoupled
components and systems. If you have nix and direnv setup then you can just cd
into this workspace and have all the necessary tools available.

## game navigation and modes

- on startup the player is presented with the choices:
  - bodies (where they make their skeletons, bodies, and animations)
  - props (where you make items, clothing, etc that can attach)
  - characters (where you combine a body with props)
  - tiles (where you manage your library of tiles)
  - worlds (where you combine tiles to make a level/world)
  - play (where you choose a world and a character and then you can run around
    and look at it)

