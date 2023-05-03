use std::collections::HashSet;

use github_change_detector::{ChangeDetector, EventType, PushEvent};

#[tokio::main]
async fn main() {
    let mut client = ChangeDetector::new("lulf", "go-vex", env!("TOKEN"));
    let mut push_ids = HashSet::new();
    loop {
        if let Ok(events)  = client.wait_events(EventType::PushEvent).await {
            println!("Found {} events", events.len());
            for event in events.iter() {
                println!("Event 1: {}", event.r#type);
                if let Ok(event) = event.payload::<PushEvent>() {
                    if push_ids.contains(&event.push_id) {
                        println!("Already seen event with id {}", event.push_id);
                        continue;
                    }
                    println!("Event with push id {}", event.push_id);
                    push_ids.insert(event.push_id);
                }
            }
        }
    }
}
