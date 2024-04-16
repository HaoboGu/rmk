
use rmk_macro::rmk_main;

#[rmk_main]
fn main() {
    println!("Hello!");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_main() {
        main();
    }
}
