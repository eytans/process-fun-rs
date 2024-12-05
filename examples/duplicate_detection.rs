use process_fun::process;

#[process]
#[process]
pub fn test_function() {
    println!("This is a test function with duplicate process attributes");
}

#[test]
pub fn dummy_test() {
    assert!(true);
}
