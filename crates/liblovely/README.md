# Liblovely

This crate builds a static (.a) library designed to be bundled in other tools to integrate lovely.
Unlike the other crates, this one does not do any work to resolve or patch lua functions. It is
expected the code linking to it does this work.

## Usage

### LuaJIT

There is an included patch file for using liblovely with LuaJIT (the default lua engine for love).
This is useful because it allows building a LuaJIT, with lovely inside, when you control of the
love build (for example <https://lmm.shorty.systems>). This patch was made on default branch of
<https://github.com/LuaJIT/LuaJIT>, but will likely work on most forks/branches of LuaJIT without
change.

To use this patch file you must:

- Have a cloned git repo of LuaJIT (e.g. `git clone https://github.com/LuaJIT/LuaJIT`)
- Apply the patch (`git apply ~/path/to/lovely-injector/crates/liblovely/luajit.patch`) (Note: on subsequent runs this will fail. If you want to reapply, first run `git reset HEAD --hard` !!This will delete any uncommited changes you have made!!)
- Build lovely (`cargo build --package liblovely --release`)
- Add `LIBS="/path/to/lovely-injector/target/release/liblovely.a"` to your make command (e.g. `make -j 16 LIBS="/path/to/lovely-injector/target/release/liblovely.a"`) (Note: if you update lovely, you will need to first `make clean` for the new lovely to be linked)

This should produce the resulting binaries in the `src/` directory. Note the luajit executable
does not work right with lovely. If you want to test you should link it to love.
This can be done easily on linux by running `LD_PRELOAD=src/libluajit.so love /path/to/game`
from the luajit directory.

### Using as a library

This library is also useful for porting to platforms where you have the ability to resolve symbols
and hook functions, but doing so in rust would be too cumbersome. To make such a program you must
be able to:

- Build lovely (rust code) for your target platform.
- Resolve the symbols for the lua libraries lovely uses (you can get the list by looking at the struct LuaLib in lovely.h.
- Redirect (hook) execution of `luaL_loadbuffer` and `luaL_loadbufferx` (and still be able to call the original loadbufferx).

If you can do so, then you may do the following:

- Call `lovely_apply_patches` when luaL_loadbuffer(x) are called.
  - `lovely_apply_patches` has the same function signature as `luaL_loadbufferx`.
  - For `luaL_loadbuffer` you can pass a null pointer to the last argument.
- Call `lovely_init(luaL_loadbufferx, lualib)` when you are able to resolve and hook the required functions, before the game is loaded.
  - This function will start up lovely (load patches, create log file, etc.)
  - This function may be called multiple times, but only the first call will impact lovely's behaviour.
  - See `lovely.h` for the definition of lualib.

Lovely should now be properly injected into your game.

## Development

### Updating the header

The header is generated from a lua script. The script will automatically parse the imported Lua functions 
from sys.rs and generate a matching C Lua struct with the correct types, in the correct order.
There is a chance that when adding new functions, it may have types that the script doesn't know about
or that differ from what the c code expects. This will need to be fixed in the script.

If the functions exported from liblovely change, you'll have to modify them near the bottom of the script.

To run the script you must first cd into `crates/liblovely` and then run the script `lua gen-h.lua` or 
`luajit gen-h.lua`. This will automatically update `lovely.h` and also print the initaliation for LuaLib
for the luajit patch (nessicary for updating the luajit patch if the order/number of LuaLib changed).

### Updating the luajit patch

Any time the header is updated the luajit patch must be regenerated as it includes the header. If the function
signature of any liblovely methods that it uses change (usually this will be LuaLib) then you will also need
to update the code.

#### First time setup

Theres a few things you must do to setup the LuaJIT repo so it can be used to generate the patch. Once this is
done you do not need to redo these steps if you keep the LuaJIT dir around. Take a script cause I'm tired of
writing words.

```sh
git clone https://github.com/LuaJIT/LuaJIT
cd LuaJIT
git apply ~/path/to/lovely-injector/crates/liblovely/luajit.patch # Update this path
git add -N src/lovely.h # So git tracks it and puts it in the patch
ln -s ~/path/to/lovely-injector/target/release/liblovely.a src/liblovely.a # Used to make the compilation easier. Remeber to update the first path
```

#### Updating the files

You can now update your files. To update the lovely.h just copy the generated one from the liblovely dir.
If you updated the LuaLib, make sure to fix the **2** instances of it in `src/lib_aux.c` (search for 
`struct LuaLib lua`) with the line printed from the script.

To test your changes worked properly:

```sh
make clean && make -j 16 LIBS="./liblovely.a"

LD_PRELOAD=src/libluajit.so love ~/path/to/Balatro.exe
```

If it's all working then you can generate the patch using:

```sh
git diff > luajit.patch
```

Remember to copy it back to the liblovely folder.

If you messed up your working tree, you can run the following to reset your files the same as they would have been after the first time setup

```sh
git reset HEAD --hard
git apply ~/path/to/lovely-injector/crates/liblovely/luajit.patch # Update this path (or use the one in the PWD if it's good)
git add -N src/lovely.h # So git tracks it and puts it in the patch
```
