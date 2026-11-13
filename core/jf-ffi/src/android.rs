//! Android: hands the Java VM and the application `Context` to `ndk-context`, which the audio
//! output (cpal's AAudio host) reads before it opens a stream; without it the first playback
//! panics. JNA loads the library without telling Rust about the VM, so the app calls
//! `org.jellyfesh.ffi.AndroidContext.initialize(applicationContext)` once, before playing
//! (`docs/ffi.md`).
//!
//! The function is exported on every platform: elsewhere nothing reads the context, and the JVM
//! tests can call it through the same JNI path as Android.

use std::ptr;
use std::sync::{Mutex, PoisonError};

use jni_sys::{JNI_FALSE, JNI_OK, JNI_TRUE, JNIEnv, JavaVM, jboolean, jclass, jobject};

/// Whether the context has been handed over; `ndk-context` accepts it only once.
static INITIALIZED: Mutex<bool> = Mutex::new(false);

/// `AndroidContext.nativeInitialize(context)`: stores the VM and a global reference to `context`
/// in `ndk-context`. Returns true when the context is set (now or by an earlier call).
///
/// # Safety
///
/// Called by the JVM through JNI: `env` is the calling thread's JNI environment and `context`
/// a local reference valid for the call.
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_jellyfesh_ffi_AndroidContext_nativeInitialize(
    env: *mut JNIEnv,
    _class: jclass,
    context: jobject,
) -> jboolean {
    let mut initialized = INITIALIZED.lock().unwrap_or_else(PoisonError::into_inner);
    if *initialized {
        return JNI_TRUE;
    }
    if env.is_null() || context.is_null() {
        return JNI_FALSE;
    }
    // SAFETY: `env` is a valid JNI environment for this thread (JNI calling convention), so its
    // function table is readable and `GetJavaVM` and `NewGlobalRef` (JNI 1.1) may be called.
    // The global reference keeps `context` alive for the life of the process, as `ndk-context`
    // requires, and the lock makes this the only `initialize_android_context` call.
    *initialized = unsafe {
        let functions = &**env;
        let mut vm: *mut JavaVM = ptr::null_mut();
        if (functions.v1_1.GetJavaVM)(env, &mut vm) != JNI_OK || vm.is_null() {
            return JNI_FALSE;
        }
        let global = (functions.v1_1.NewGlobalRef)(env, context);
        if global.is_null() {
            return JNI_FALSE;
        }
        ndk_context::initialize_android_context(vm.cast(), global.cast());
        true
    };
    JNI_TRUE
}
