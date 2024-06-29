# Lovely is a runtime lua injector for LÖVE 2d

Lovely is a lua injector which embeds code into a [LÖVE 2d](https://love2d.org/) game at runtime. Unlike executable patchers, mods can be installed, updated, and removed *over and over again* without requiring a partial or total game reinstallation. This is accomplished through in-process lua API detouring and an easy to use (and distribute) patch system.

## Manual Installation

### Windows + Proton / Wine

1. Download the [latest release](https://github.com/ethangreen-dev/lovely-injector/releases) for Windows. This will be `lovely-x86_64-pc-windows-msvc.zip`.
2. Open the .zip archive, copy `version.dll` into the game directory. You can navigate to the game's directory by right-clicking the game in Steam, hovering "Manage", and selecting "Browse local files".
3. Put one or more mods into the mod directory (NOT the same as the game directory). This should be `%AppData%/Balatro/Mods` (if you are modding Balatro).
4. **<ins>Only Steam Deck / Proton / Wine</ins>** Set your game's launch options in Steam to `WINEDLLOVERRIDES="version=n,b" %command%`.
5. Run the game through Steam.

### Mac

1. Download the [latest release](https://github.com/ethangreen-dev/lovely-injector/releases) for Mac. If you have an M-series CPU (M1, M2, etc.) then this will be `lovely-aarch64-apple-darwin.tar.gz`. If you have an Intel CPU then it will be `lovely-x86_64-apple-darwin.tar.gz`
2. Open the .zip archive, copy `liblovely.dylib` and `run_lovely.sh` into the game directory. You can navigate to the game's directory by right-clicking the game in Steam, hovering "Manage", and selecting "Browse local files".
3. Put one or more mods into the Mac mod directory (NOT the same as the game directory). This should be `/Users/$USER/Library/Application Support/Balatro/Mods` where `$USER` is your username (if you are modding Balatro).\
If you can't find this folder, try pressing `Shift-Command-.` (period) to show hidden files in Finder.
4. Run the game by either dragging and dropping `run_lovely.sh` onto `Terminal.app` in Applications > Utilities and then pressing enter, or by executing `sh run_lovely.sh` in the terminal within the game directory.

Note: You cannot run your game through Steam on Mac due to a bug within the Steam client. You must run it with the `run_lovely.sh` script.

**Important**: Mods with Lovely patch files (`lovely.toml` or in `lovely/*.toml`) **must** be installed into their own folder within the mod directory. No exceptions!

## Patches

*Note that the patch format is unstable and prone to change until Lovely is out of early development.*

*Patch files* define where and how code injection occurs within the game process. A good (complex) example of this can be found in the Steamodded repo [here](https://github.com/Steamopollys/Steamodded/tree/main/lovely).
```toml
[manifest]
version = "1.0.0"
priority = 0

# Define a var substitution rule. This searches for lines that contain {{lovely:var_name}} 
# (var_name from this example, it can really be anything) and replaces each match with the 
# provided value.
# This example would transform print('{{lovely:var_name}}') to print('Hello world!').
# 
# USEFUL: For when you want to reduce the complexity of repetitive injections, eg. embedding 
# release version numbers in multiple locations.
[vars]
var_name = "Hello world!"

# Inject one or more lines of code before, after, or at (replacing) a line which matches 
# the provided pattern.
#
# USEFUL: For when you need to add / modify a small amount of code to setup initialization 
# routines, etc.
[[patches]]
[patches.pattern]
target = "game.lua"
pattern = "self.SPEEDFACTOR = 1"
position = "after"
payload = '''
initSteamodded()
print('{{lovely:var_name}}')
'''
match_indent = true
times = 1

# Inject one or more lines of code before, after, at, or interwoven into one or more 
# Regex capture groups.
# - I recommend you to use a Regex playground like https://regexr.com to build 
#   your patterns.
# - Regex is NOT EFFICIENT. Please use the pattern patch unless absolutely necessary.
# - This patch has capture group support.
# - This patch does NOT trim whitespace from each line. Take that into account when 
#   designing your pattern.
#
# USEFUL: For when the pattern patch is not expressive enough to describe how the 
# payload should be injected.
[patches.regex]
target = "tag.lua"
pattern = "(?<indent>[\t ]*)if (?<cond>_context.type == 'eval' then)"
position = 'at'
line_prepend = '$indent'
payload = '''
local obj = SMODS.Tags[self.key]
local res
if obj and obj.apply and type(obj.apply) == 'function' then
    res = obj.apply(self, _context)
end
if res then
    return res
elseif $cond
'''
times = 1

# Append or prepend the contents of one or more files onto the target.
#
# USEFUL: For when you *only* care about getting your code into the game, nothing else. 
# This does NOT inject it as a new module.
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

# Inject a new module into the game *before* a target file it loaded.
# USEFUL: For when you want to silo your code into a separate require-able module OR inject a "global" dependency before game / mod code begins execution.
[[patches]]
[patches.module]
source = "nativefs.lua"
before = "main.lua"
name = "nativefs"
```

### TL;DR - Patch variants

- Use `pattern` patches to surgically embed code at specific locations within the target. Supports `*` (matches 0 or more occurrences of any character) and `?` (matches exactly one occurrence of any character) wildcards.
- Use `regex` patches *only* when the pattern patch does not fulfill your needs. This is basically the pattern patch but with a backing regex query engine, capture groups and all.
- Use `copy` patches when you need to copy a large amount of position-independent code into the target.
- Use `module` patches to inject a lua module into the game's runtime. Note that this currently only supports single file modules, but this should be changing soon.

### Patch files

Patch files are loaded from mod directories inside of the mod folder (`MOD_DIR`). Lovely will load any patch files present within `MOD_DIR/ModName/lovely/` or load a single patch from `MOD_DIR/ModName/lovely.toml`. If multiple patches are loaded they will be injected into the game in the order in which they are found.

Paths defined within the patch are rooted by the mod's directory. For example, `core/deck.lua` resolves to `MOD_DIR/ModName/core/deck.lua`.

### Patch targets

Each patch definition has a single patch target. These targets are the relative paths of source files when dumped from the game with a tool like 7zip. For example, one can target a top-level file like `main.lua`, or one in a subdirectory like `engine/event.lua`.

### Patch debugging

Lovely dumps patched lua source files to `MOD_DIR/lovely/dump`. Logs are likewise written to `MOD_DIR/lovely/log`.

## Not yet implemented

- `manifest.version`
