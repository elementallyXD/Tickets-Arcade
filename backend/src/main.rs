use std::io;

fn main() {
    // Prompt the user for their name
    println!("Please enter your name:");

    // Create a mutable String to hold the user input
    let mut user_name: String = String::new();

    // Read the user's input from stdin
    io::stdin()
        .read_line(&mut user_name)
        .expect("Failed: Unable to read line.");

    // Trim newline and print a greeting message
    let user_name = user_name.trim();
    println!("Hello, world: {}!", user_name);

    // Check if the user is "admin"
    if check_name(user_name) {
        println!("This is admin!");
    }
}

// Function to check if the user is "admin"
// Return true if the user is "admin", otherwise false
fn check_name(user: &str) -> bool {
    matches!(
        user,
        "admin" | "administrator" | "root" | "superuser" | "sysadmin"
    )
}
