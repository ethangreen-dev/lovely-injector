# Lovely is a runtime lua injector for LÖVE 2d

Lovely is a lua injector which embeds code into a [LÖVE 2d](https://love2d.org/) game at runtime. Unlike executable patchers, mods can be installed, updated, and removed *over and over again* without requiring a partial or total game reinstallation. This is accomplished through in-process lua API detouring and an easy to use (and distribute) patch system.

## Installation

1. Download the latest release.
2. Open the .zip archive, copy `dwmapi.dll` into the game directory. You can navigate to the location by right-clicking the game in Steam, hovering "Manage", and selecting "Browse local files".
3. Install one or more mods into `%appdata%/Balatro/Mods`.
4. Run the game.

**Important**: Mods with Lovely patch files (`lovely.toml` or in `lovely/*.toml`) **must** be installed into their own directory within `%AppData%/Balatro/Mods`. No exceptions!

## Patches

*Note that the patch format is unstable and prone to change until Lovely is out of early development.*

*Patch files* define where and how code injection occurs within the game process. For example, this is a patch for the modloader Steamodded:

```toml
[manifest]
version = "1.0.0"
dump_lua = true
priority = 0

[[patches]]
[patches.pattern]
target = "game.lua"
pattern = "self.SPEEDFACTOR = 1"
position = "after"
payload = "initSteamodded()"
match_indent = true
overwrite = false

[[patches]]
[patches.copy]
target = "main.lua"
position = "append"
sources = [
    "core/core.lua",
    "core/deck.lua",
    "core/joker.lua",
    "core/sprite.lua",
    "debug/debug.lua",
    "loader/loader.lua",
]
```
### Patch variants 

This file contains two patch definitions - a pattern patch, which (currently) changes a single line at a position offset to some pattern match, and a copy patch, which reads one or more input lua files and either appends or prepends them onto the target. The former is used when you need to surgically embed code at specific locations in the target (very useful for modloader init routines), and the latter is designed for use when you need to bulk inject position-independent code into the game.

### Patch files

Patch files are loaded from mod directories inside of `%AppData%/Balatro/Mods`. Lovely will load any patch files present within `Mods/ModName/lovely/` or load a single patch from `%AppData/Balatro/Mods/ModName/lovely.toml`. If multiple patches are loaded they will be injected into the game in the order in which they are found.

Paths defined within the patch are rooted by the mod's directory. For example, `core/deck.lua` is resolved to `%AppData%/Balatro/Steamodded/core/deck.lua`.

### Patch targets

Each patch definition has a single patch target. These targets are the relative paths of source files when dumped from the game with a tool like 7zip. For example, one can target a top-level file like `main.lua`, or one in a subdirectory like `engine/event.lua`.

### Patch debugging

Lovely dumps patched lua source files to `%AppData%/Balatro/Mods/lovely/dump`.

## Not yet implemented

- `manifest.priority`
- `manifest.dump_lua`
- `manifest.version`
