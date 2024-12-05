use serde::{Serialize, Deserialize};

pub use process_fun_macro::process;
pub use process_fun_core::generate_unique_hash;

#[derive(Serialize, Deserialize)]
pub struct Point {
    pub x: i32,
    pub y: i32,
}

#[process]
pub fn add_points(p1: Point, p2: Point) -> Point {
    Point {
        x: p1.x + p2.x,
        y: p1.y + p2.y,
    }
}

/// Initialize the process-fun runtime. This should be called at the start of your main function.
/// It will check for the special process hash argument and if present, execute the corresponding function.
#[macro_export]
macro_rules! init_process_fun {
    () => {
        let args: Vec<String> = std::env::args().collect();
        if args.len() >= 2 {
            if let Some(hash) = args.get(1) {
                if let Some(args_json) = args.get(2) {
                    // Find and execute the function with matching hash
                    for func in inventory::iter::<process_fun_core::ProcessFunction> {
                        if **func.hash == *hash {
                            if let Some(result) = (func.handler)(args_json.clone()) {
                                println!("{}", result);
                                std::process::exit(0);
                            }
                        }
                    }
                }
            }
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_process_function() {
        init_process_fun!();
        let p1 = Point { x: 1, y: 2 };
        let p2 = Point { x: 3, y: 4 };

        let result = add_points_process(p1, p2).unwrap();
        assert_eq!(result.x, 4);
        assert_eq!(result.y, 6);
    }
}
