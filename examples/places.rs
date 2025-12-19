//! Example: List common places
//!
//! This example demonstrates how to use the PlacesService to get common directory locations.

use npio::service::places::PlacesService;

fn main() {
    let service = PlacesService::new();
    let places = service.get_common_places();

    println!("Common Places:");
    for place in places {
        println!("  {} ({}) - {}", place.name, place.icon, place.file);
    }
}

