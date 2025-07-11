mod lualib;

use lovely_core::{log::*, config};
use lovely_core::sys::LuaState;
use lualib::LUA_LIBRARY;
use std::path::PathBuf;
use std::{ffi::c_void, mem, panic, sync::{LazyLock, OnceLock}};


use jni::{JNIEnv, JNIVersion, JavaVM};
use jni::objects::JString;
use jni::sys::{jint, jvalue};

use lovely_core::Lovely;

static RUNTIME: OnceLock<Lovely> = OnceLock::new();

static RECALL: LazyLock<
    unsafe extern "C" fn(*mut LuaState, *const u8, usize, *const u8, *const u8) -> u32,
> = LazyLock::new(|| unsafe {
    let lua_loadbufferx: unsafe extern "C" fn(
        *mut LuaState,
        *const u8,
        usize,
        *const u8,
        *const u8,
    ) -> u32 = *LUA_LIBRARY.get(b"luaL_loadbufferx").unwrap();
    let orig = dobby_rs::hook(
        lua_loadbufferx as *mut c_void,
        lua_loadbufferx_detour as *mut c_void,
    )
    .unwrap();
    mem::transmute(orig)
});

unsafe extern "C" fn lua_loadbufferx_detour(
    state: *mut LuaState,
    buf_ptr: *const u8,
    size: usize,
    name_ptr: *const u8,
    mode_ptr: *const u8,
) -> u32 {
    let rt = RUNTIME.get().unwrap_unchecked();
    rt.apply_buffer_patches(state, buf_ptr, size, name_ptr, mode_ptr)
}

#[no_mangle]
#[allow(non_snake_case)]
unsafe extern "C" fn luaL_loadbuffer(
    state: *mut LuaState,
    buf_ptr: *const u8,
    size: usize,
    name_ptr: *const u8,
) -> u32 {
    let rt = RUNTIME.get().unwrap_unchecked();
    rt.apply_buffer_patches(state, buf_ptr, size, name_ptr, std::ptr::null())
}

unsafe fn get_external_files_dir(env: &mut JNIEnv) -> Result<PathBuf, jni::errors::Error> {
    let activity_thread_class = env.find_class("android/app/ActivityThread")?;
    let context_class = env.find_class("android/content/Context")?;
    let external_files_dir_method = env.get_method_id(context_class, "getExternalFilesDir", "(Ljava/lang/String;)Ljava/io/File;")?;

    let activity_thread = env.call_static_method(activity_thread_class, "currentActivityThread", "()Landroid/app/ActivityThread;", &[])?.l()?;
    let context = env.call_method(activity_thread, "getApplication", "()Landroid/app/Application;", &[])?.l()?;
    let external_files_dir = env.call_method_unchecked(context, external_files_dir_method, jni::signature::ReturnType::Object, &[jvalue{l: std::ptr::null_mut()}])?.l()?;
    let external_files_dir_string: JString = env.call_method(external_files_dir, "getAbsolutePath", "()Ljava/lang/String;", &[])?.l()?.into();
    let utf8 = env.get_string(&external_files_dir_string)?;
    
    Ok(PathBuf::from(utf8.to_str().unwrap()))
}

#[allow(non_snake_case)]
#[no_mangle]
unsafe extern "C" fn JNI_OnLoad(jvm: JavaVM, _: *mut c_void) -> jint {    
    panic::set_hook(Box::new(|x| {
        let message = format!("lovely-injector has crashed: \n{x}");
        error!("{message}");
    }));

    let mut env = jvm.get_env().unwrap();
    let external_files_dir = get_external_files_dir(&mut env).expect("Failed to get external files directory.");
    let config = config::LovelyConfig {
        dump_all: false,
        vanilla: false,
        mod_dir: Some(external_files_dir.join("mods")),
    };
    
    let rt = Lovely::init(&|a, b, c, d, e| RECALL(a, b, c, d, e), lualib::get_lualib(), config);
    RUNTIME
        .set(rt)
        .unwrap_or_else(|_| panic!("Failed to instantiate runtime."));

    let lua_loadbuffer: unsafe extern "C" fn(
        *mut LuaState,
        *const u8,
        isize,
        *const u8,
    ) -> u32 = *LUA_LIBRARY.get(b"luaL_loadbuffer").unwrap();
    let _ = dobby_rs::hook(
        lua_loadbuffer as *mut c_void,
        luaL_loadbuffer as *mut c_void,
    )
    .unwrap();

    JNIVersion::V4.into()
}
