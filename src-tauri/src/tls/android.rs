//! Hands the VM and the application context to `rustls-platform-verifier`.

use jni::objects::JObject;
use jni::JavaVM;

use super::InitError;

pub(super) fn init() -> Result<(), InitError> {
    let context = ndk_context::android_context();
    // SAFETY: tao publishes the VM and the application context in `onCreate`, before Tauri runs
    // `setup`; both stay valid for the life of the process.
    let vm = unsafe { JavaVM::from_raw(context.vm().cast()) };
    vm.attach_current_thread(|env| -> Result<(), jni::errors::Error> {
        // SAFETY: a global reference held by tao for the life of the process; the wrapper
        // never deletes it.
        let app_context = unsafe { JObject::from_raw(env, context.context().cast()) };
        rustls_platform_verifier::android::init_with_env(env, app_context)
    })
    .map_err(|error| InitError(error.to_string()))
}
