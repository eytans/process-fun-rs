use process_fun::process;
use serde::{Serialize, Deserialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
struct Point {
    x: i32,
    y: i32,
}

// Example showing basic process function usage
#[process]
fn calculate_distance(p1: Point, p2: Point) -> f64 {
    let dx = (p2.x - p1.x) as f64;
    let dy = (p2.y - p1.y) as f64;
    (dx * dx + dy * dy).sqrt()
}

// Example showing mutable data behavior
#[process]
fn move_point(mut point: Point, dx: i32, dy: i32) -> Point {
    point.x += dx;
    point.y += dy;
    point  // Changes only affect returned value
}

fn main() {
    // Initialize process-fun
    process_fun::init_process_fun!();

    println!("Running examples with debug prints enabled...\n");

    // Basic distance calculation example
    let p1 = Point { x: 0, y: 0 };
    let p2 = Point { x: 3, y: 4 };

    println!("Calculating distance between {:?} and {:?}...", p1, p2);
    match calculate_distance_process(p1.clone(), p2.clone()) {
        Ok(distance) => println!("Distance: {}", distance), // Should print: Distance: 5.0
        Err(e) => eprintln!("Error: {}", e),
    }
    println!();

    // Mutable data example
    let original = Point { x: 10, y: 20 };
    println!("Moving point with mutable data example:");
    println!("Original point: {:?}", original);

    // Move point in separate process
    match move_point_process(original.clone(), 5, 5) {
        Ok(moved) => {
            // Original point remains unchanged
            println!("Original point (unchanged): {:?}", original);
            println!("Moved point (new): {:?}", moved);
        }
        Err(e) => eprintln!("Error: {}", e),
    }
}
