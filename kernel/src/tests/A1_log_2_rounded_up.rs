use crate::println;
use kernel_test::{kernel_test, kernel_test_mod};
kernel_test_mod!(crate::tests::A1_log_2_rounded_up);

#[kernel_test]
fn test_log_2_rounded_up() -> bool {
    let mut passed = true;
    for i in 1..1024 {
        let res = log2_rounded_up(i);
        if 2_u64.pow(res as u32) < i || (res > 0 && 2_u64.pow(res as u32 - 1) >= i) {
            println!("log2_rounded_up({}) = {} is incorrect", i, res);
            passed = false;
        }
    }
    passed
}
