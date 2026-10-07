use super::init_platform_verifier;

#[test]
fn init_is_a_no_op_off_android_and_can_run_twice() {
    assert!(init_platform_verifier().is_ok());
    assert!(init_platform_verifier().is_ok());
}
