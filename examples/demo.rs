use const_to_static_table::{LazyInitializer, initialize_all};

static INIT1: LazyInitializer<i32> = LazyInitializer::new(&|| async move { 1 });
static INIT2: LazyInitializer<i32> = LazyInitializer::new(&|| async move { 2 });
static INIT3: LazyInitializer<i32> = LazyInitializer::new(&|| async move { 3 });
static INIT4: LazyInitializer<i32> = LazyInitializer::new(&|| async move { 42 });
static DB: LazyInitializer<Database> =
    LazyInitializer::new(&|| async move { Database::new().await });

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
