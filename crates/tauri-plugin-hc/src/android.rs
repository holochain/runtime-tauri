//! Android: initialize [`ndk_context`] when the native library loads.
//!
//! Crates in the conductor's dependency tree reach Android APIs through
//! `ndk_context::android_context()`: iroh's DNS resolver (via hickory) reads the
//! system nameservers with it, `netdev` enumerates interfaces with it, and apps
//! commonly resolve directories with `app_dirs2`, which uses it too. Tauri used to
//! initialize it through tao's `ndk_glue`; tao 0.35 no longer does, so the first
//! call panics ("android context was not initialized"). A panic that crosses a JNI
//! frame aborts the app, and with `panic = "abort"` even iroh's own `catch_unwind`
//! around the DNS lookup cannot recover.
//!
//! `JNI_OnLoad` runs when the activity calls `System.loadLibrary`, before any of
//! the app's Rust code, on a thread already attached to the VM. The context is the
//! process's `Application`, which outlives every activity. If anything here fails,
//! the context stays uninitialized and a warning is logged; loading continues.
//!
//! Only one `JNI_OnLoad` can exist per native library. An app that defines its own
//! will fail to link with a duplicate-symbol error, and should call
//! [`ndk_context::initialize_android_context`] from it instead.

use jni::objects::JObject;
use jni::sys::{jint, JNI_VERSION_1_6};
use jni::JavaVM;
use std::ffi::c_void;
use std::panic::{catch_unwind, AssertUnwindSafe};

#[no_mangle]
pub extern "system" fn JNI_OnLoad(vm: JavaVM, _reserved: *mut c_void) -> jint {
    match catch_unwind(AssertUnwindSafe(|| install_application_context(&vm))) {
        Ok(Ok(())) => {}
        Ok(Err(e)) => log::warn!("tauri-plugin-hc: could not initialize ndk_context: {e}"),
        Err(_) => log::warn!("tauri-plugin-hc: panic while initializing ndk_context"),
    }
    JNI_VERSION_1_6
}

fn install_application_context(vm: &JavaVM) -> jni::errors::Result<()> {
    let mut env = vm.get_env()?;
    let application = env
        .call_static_method(
            "android/app/ActivityThread",
            "currentApplication",
            "()Landroid/app/Application;",
            &[],
        )?
        .l()?;
    if application.is_null() {
        return Err(jni::errors::Error::NullPtr(
            "ActivityThread.currentApplication()",
        ));
    }
    install(vm, &mut env, application)
}

fn install(vm: &JavaVM, env: &mut jni::JNIEnv, context: JObject) -> jni::errors::Result<()> {
    // A global reference the process never releases: ndk_context hands out the raw
    // pointer for as long as the library is loaded.
    let context = env.new_global_ref(context)?;
    let context_ptr = context.as_obj().as_raw() as *mut c_void;
    std::mem::forget(context);
    // SAFETY: the VM pointer is valid for the life of the process, and the context
    // pointer is a global reference that is deliberately never deleted.
    unsafe {
        ndk_context::initialize_android_context(
            vm.get_java_vm_pointer() as *mut c_void,
            context_ptr,
        );
    }
    Ok(())
}
