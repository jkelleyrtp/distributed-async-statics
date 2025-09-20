use const_to_static_table::{Lazy, initialize_all};

static INIT1: Lazy<i32> = Lazy::new(|| async { 1 });
static INIT2: Lazy<i32> = Lazy::new(|| async { 2 });
static INIT3: Lazy<i32> = Lazy::new(|| async { 3 });
static INIT4: Lazy<i32> = Lazy::new(|| async { 42 });
static DB: Lazy<Database> = Lazy::new(|| async { Database::new().await });

struct Database {}
impl Database {
    async fn new() -> Self {
        Database {}
    }
    async fn say_hi(&self) {
        println!("hi from db");
    }
}

#[tokio::main]
async fn main() {
    initialize_all().await;

    println!("INIT1: {}", INIT1.get());
    println!("INIT2: {}", INIT2.get());
    println!("INIT3: {}", INIT3.get());
    println!("INIT4: {}", INIT4.get());

    DB.say_hi().await;
}
