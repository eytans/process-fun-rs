use process_fun::process;
use serde::{Serialize, Deserialize};

#[derive(Debug, Serialize, Deserialize)]
struct Point {
    x: i32,
    y: i32,
}

#[process]
fn calculate_distance(p1: Point, p2: Point) -> f64 {
    let dx = (p2.x - p1.x) as f64;
    let dy = (p2.y - p1.y) as f64;
    (dx * dx + dy * dy).sqrt()
}

#[tokio::main]
async fn main() {
    // Initialize process-fun
    process_fun::init_process_fun!();

    // Create test points
    let p1 = Point { x: 0, y: 0 };
    let p2 = Point { x: 3, y: 4 };

    // Calculate distance in a separate process
    match calculate_distance_async(p1, p2).await {
        Ok(distance) => println!("Distance: {}", distance), // Should print: Distance: 5.0
        Err(e) => eprintln!("Error: {}", e),
    }
}
