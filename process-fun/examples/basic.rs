use process_fun::process;
use serde::{Deserialize, Serialize};
use std::thread;
use std::time::Duration;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Point {
    x: i32,
    y: i32,
}

// Example showing basic process function usage
#[process]
pub fn calculate_distance(p1: Point, p2: Point) -> f64 {
    let dx = (p2.x - p1.x) as f64;
    let dy = (p2.y - p1.y) as f64;
    (dx * dx + dy * dy).sqrt()
}

// Example showing mutable data behavior
#[process]
#[allow(unused_mut)]
pub fn move_point(mut point: Point, dx: i32, dy: i32) -> Point {
    point.x += dx;
    point.y += dy;
    point // Changes only affect returned value
}

// Example showing timeout functionality
#[process]
pub fn slow_calculation(iterations: u64) -> u64 {
    let mut sum: u64 = 0;
    for i in 0..iterations {
        sum = sum.wrapping_add(i as u64);
        if i % 1000 == 0 {
            thread::sleep(Duration::from_micros(1));
        }
    }
    sum
}

fn main() {
    println!("Running examples with debug prints enabled...\n");

    // Basic distance calculation example
    let p1 = Point { x: 0, y: 0 };
    let p2 = Point { x: 3, y: 4 };

    println!("Calculating distance between {:?} and {:?}...", &p1, &p2);
    let mut process = calculate_distance_process(p1.clone(), p2.clone()).unwrap();
    match process.timeout(Duration::from_secs(1)) {
        Ok(distance) => println!("Distance: {}", distance), // Should print: Distance: 5.0
        Err(e) => panic!("Error: {}", e),
    }
    println!();

    // Mutable data example
    let original = Point { x: 10, y: 20 };
    println!("Moving point with mutable data example:");
    println!("Original point: {:?}", &original);

    // Move point in separate process
    let mut process = move_point_process(original.clone(), 5, 5).unwrap();
    match process.timeout(Duration::from_secs(1)) {
        Ok(moved) => {
            // Original point remains unchanged
            println!("Original point (unchanged): {:?}", original);
            println!("Moved point (new): {:?}", moved);
        }
        Err(e) => panic!("Error: {}", e),
    }
    println!();

    // Timeout example
    println!("Running slow calculation with timeout...");
    let mut process = slow_calculation_process(10_000_000).unwrap();
    match process.timeout(Duration::from_millis(100)) {
        Ok(result) => println!("Calculation completed with result: {}", result),
        Err(e) => println!("Calculation timed out as expected: {}", e),
    }
}
