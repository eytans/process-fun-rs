use process_fun::process;

#[process]
pub fn test_function2() {
    println!("This is a test function");
}

// The build script will verify that it can find this function
// No actual test needed here since the build script will fail
// if it doesn't find any #[process] functions
#[test]
pub fn dummy_test() {
    assert!(true);
}
