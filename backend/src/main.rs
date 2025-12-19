use std::io;

fn main() {
    println!("Please enter your name:");

    let mut user_name: String = String::new();

    io::stdin()
        .read_line(&mut user_name)
        .expect("Failed: Unable to read line.");

    let user_name = user_name.trim();
    println!("Hello, : {}", user_name);

    if check_name(user_name) {
        println!("This is admin!");
    }
}

// Function to check if the user name is an admin name
// Returns true if the name matches any of the predefined admin names
fn check_name(user: &str) -> bool {
    matches!(
        user,
        "admin" | "administrator" | "root" | "superuser" | "sysadmin"
    )
}
