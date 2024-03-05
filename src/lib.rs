mod manifest;
mod patch;

use std::collections::HashMap;
use std::{env, fs};
use std::ffi::{c_void, CStr, CString};
use std::path::PathBuf;

use getargs::{Arg, Options};
use retour::static_detour;

use windows::core::{s, w};
use windows::Win32::System::Console::AllocConsole;
use windows::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryW};

use once_cell::sync::OnceCell;

use crate::manifest::{CopyAt, CopyPatch, Manifest, Patch, PatchManifest, PatternAt, PatternPatch};

static LOADER_DIR: OnceCell<PathBuf> = OnceCell::new();
static MOD_DIR: OnceCell<PathBuf> = OnceCell::new();

static_detour! {
    pub static LuaLoadbuffer_Detour: unsafe extern "C" fn(*mut c_void, *const u8, isize, *const u8);
}

unsafe extern "C" fn lua_loadbuffer_detour(lua_state: *mut c_void, buff: *const u8, size: isize, name_buf: *const u8) {
    println!("entry");
    let name = CStr::from_ptr(name_buf as _).to_str().unwrap();

    if !patch::is_patch_target(name) {
        return LuaLoadbuffer_Detour.call(lua_state, buff, size, name_buf);
    }

    let input_buf = std::slice::from_raw_parts(buff, size as _);
    println!("{input_buf:?}\n");

    let input = CString::from_vec_unchecked(input_buf.to_vec());
    let input = input.to_str().unwrap().to_string();

    let patched = patch::apply(input.as_str(), name);
    if patched.is_none() {
        return LuaLoadbuffer_Detour.call(lua_state, buff, size, name_buf);
    }

    let patched = patched.unwrap();
    let path = format!("patch-{name}");
    fs::write(path, &patched).unwrap();

    let raw = CString::new(patched).unwrap();
    let raw_nul = raw.as_bytes();

    LuaLoadbuffer_Detour.call(lua_state, raw_nul.as_ptr(), raw_nul.len() as _, name_buf)
}

#[no_mangle]
#[allow(non_snake_case)]
unsafe extern "system" fn DllMain(_: *mut c_void, reason: u32, _: *const c_void) -> u8 {
    AllocConsole();

    let test = PatchManifest {
        manifest: Manifest {
            version: "1".into(),
            dump_lua: false,
            priority: 0
        },
        patches: vec![
            Patch::Pattern(PatternPatch {
                pattern: "pattern".into(),
                position: PatternAt::Before,
                target: "@thing.lua".into(),
                payload_files: None,
                payload: Some("something".into()),
                match_indent: true,
                overwrite: false
            }),
            Patch::Copy(CopyPatch {
                position: CopyAt::Append,
                target: "thing".into(),
                sources: vec!["thing".into()],
            })
        ]
    };

    fs::write("test.toml", toml::to_string_pretty(&test).unwrap()).unwrap();

    if reason != 1 {
        return 1;
    }

    let patch: PatchManifest = {
        let path = env::current_dir().unwrap().join("patches/patch.toml");
        let str = fs::read_to_string(&path).unwrap_or_else(|_| panic!("Failed to read patch file at '{path:?}'"));

        toml::from_str(&str).unwrap_or_else(|e| panic!("Failed to parse patch file at '{path:?}'. Error: {e:?}"))
    };

    let patches = patch.patches;
    let targets = patches.iter().map(|x| match x {
        Patch::Pattern(x) => x.target.as_str(),
        Patch::Copy(x) => x.target.as_str(),
    });

    let mut table: HashMap<String, Vec<usize>> = HashMap::new();
    for (i, target) in targets.enumerate() {
        let target = target.to_string();
        if !table.contains_key(&target) {
            table.insert(target.to_string(), vec![i]);
            continue;
        }

        let vals = table.get_mut(&target).unwrap();
        vals.push(i);
    }

    dbg!(&patches);
    dbg!(&table);

    patch::PATCHES.set(patches).unwrap();
    patch::PATCH_TABLE.set(table).unwrap();

    // let args = std::env::args().skip(1).collect::<Vec<_>>();
    // let mut opts = Options::new(args.iter().map(String::as_str));

    // while let Some(opt) = opts.next_arg().expect("Failed to parse argument.") {
    //     match opt {
    //         Arg::Long("loader-dir") => LOADER_DIR.set(PathBuf::from(opts.value().expect("`--loader-dir` argument has no value."))).unwrap(),
    //         Arg::Long("mod-dir") => MOD_DIR.set(PathBuf::from(opts.value().expect("`--mod-dir` argument has no value."))).unwrap(),
    //         _ => (),
    //     }
    // }

    let handle = LoadLibraryW(w!("lua51.dll")).unwrap();
    let proc = GetProcAddress(handle, s!("luaL_loadbuffer")).unwrap();
    let fn_target = std::mem::transmute::<_, unsafe extern "C" fn(*mut c_void, *const u8, isize, *const u8)>(proc);

    LuaLoadbuffer_Detour.initialize(
        fn_target, 
        |a, b, c, d| lua_loadbuffer_detour(a, b, c, d)
    )
    .unwrap()
    .enable()
    .unwrap();

    1
}
