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
        let raw_context: jni::sys::jobject = context.context().cast();
        // SAFETY: a valid global reference that tao holds for the life of the process; it is
        // only borrowed here, and the verifier gets a local reference of its own.
        let borrowed = unsafe { env.as_cast_raw::<JObject>(&raw_context)? };
        let app_context = env.new_local_ref(&*borrowed)?;
        rustls_platform_verifier::android::init_with_env(env, app_context)
    })
    .map_err(|error| InitError(error.to_string()))
}
