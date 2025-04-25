use lovely_core::{log::*, LovelyConfig};
use lovely_core::sys::{LuaState, LUA_LIB};
use std::path::PathBuf;
use std::{ffi::c_void, mem, panic, sync::{LazyLock, OnceLock}};


use jni::{JNIEnv, JNIVersion, JavaVM};
use jni::objects::JString;
use jni::sys::{jint, jvalue};

use lovely_core::Lovely;

static RUNTIME: OnceLock<Lovely> = OnceLock::new();

static RECALL: LazyLock<
    unsafe extern "C" fn(*mut LuaState, *const u8, isize, *const u8, *const u8) -> u32,
> = LazyLock::new(|| unsafe {
    let lua_loadbufferx: unsafe extern "C" fn(
        *mut LuaState,
        *const u8,
        isize,
        *const u8,
        *const u8,
    ) -> u32 = *LUA_LIB.get(b"luaL_loadbufferx").unwrap();
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
    size: isize,
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
    size: isize,
    name_ptr: *const u8,
) -> u32 {
    let rt = RUNTIME.get().unwrap_unchecked();
    rt.apply_buffer_patches(state, buf_ptr, size, name_ptr, std::ptr::null())
}

#[allow(non_snake_case)]
unsafe fn get_external_files_dir(env: &mut JNIEnv) -> PathBuf {
    let activityThreadClass = env.find_class("android/app/ActivityThread").unwrap();
    let contextClass = env.find_class("android/content/Context").unwrap();
    let externalFilesDirMethod = env.get_method_id(contextClass, "getExternalFilesDir", "(Ljava/lang/String;)Ljava/io/File;").unwrap();

    let activityThread = env.call_static_method(activityThreadClass, "currentActivityThread", "()Landroid/app/ActivityThread;", &[]).unwrap().l().unwrap();
    let context = env.call_method(activityThread, "getApplication", "()Landroid/app/Application;", &[]).unwrap().l().unwrap();
    let externalFilesDir = env.call_method_unchecked(context, externalFilesDirMethod, jni::signature::ReturnType::Object, &[jvalue{l: std::ptr::null_mut()}]).unwrap().l().unwrap();
    let externFilesDirString: JString = env.call_method(externalFilesDir, "getAbsolutePath", "()Ljava/lang/String;", &[]).unwrap().l().unwrap().into();
    let utf8 = env.get_string(&externFilesDirString).unwrap();
    PathBuf::from(utf8.to_str().unwrap())
}

#[allow(non_snake_case)]
#[no_mangle]
unsafe extern "C" fn JNI_OnLoad(jvm: JavaVM, _: *mut c_void) -> jint {    
    panic::set_hook(Box::new(|x| {
        let message = format!("lovely-injector has crashed: \n{x}");
        error!("{message}");
    }));

    let mut env = jvm.get_env().unwrap();
    let external_files_dir = get_external_files_dir(&mut env);
    let config = LovelyConfig {
        dump_all: false,
        vanilla: false,
        mod_dir: Some(external_files_dir.join("files/mods")),
    };
    
    let rt = Lovely::init(&|a, b, c, d, e| RECALL(a, b, c, d, e), config);
    RUNTIME
        .set(rt)
        .unwrap_or_else(|_| panic!("Failed to instantiate runtime."));

    let lua_loadbuffer: unsafe extern "C" fn(
        *mut LuaState,
        *const u8,
        isize,
        *const u8,
    ) -> u32 = *LUA_LIB.get(b"luaL_loadbuffer").unwrap();
    let _ = dobby_rs::hook(
        lua_loadbuffer as *mut c_void,
        luaL_loadbuffer as *mut c_void,
    )
    .unwrap();

    JNIVersion::V4.into()
}
