use const_to_static_table::{Lazy, initialize_all};

static INIT1: Lazy<String> = Lazy::new(|| async {
    reqwest::get("https://dog.ceo/api/breeds/image/random")
        .await
        .unwrap()
        .json::<serde_json::Value>()
        .await
        .unwrap()["message"]
        .as_str()
        .unwrap()
        .to_string()
});
static INIT2: Lazy<i32> = Lazy::new(|| async { 2 });
static INIT3: Lazy<f32> = Lazy::new(|| async { 3.0 });
static INIT4: Lazy<String> = Lazy::new(|| async { 42.to_string() });
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

    println!("1: {}", INIT1);
    println!("2: {}", INIT2);
    println!("3: {}", INIT3);
    println!("4: {}", INIT4);

    DB.say_hi().await;
}
