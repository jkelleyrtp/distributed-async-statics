use const_to_static_table::{LazyInitializer, initialize_all};

#[used]
static INIT1: LazyInitializer = LazyInitializer::new(&|| async move { 1 });

#[used]
static INIT2: LazyInitializer = LazyInitializer::new(&|| async move { 2 });

#[used]
static INIT3: LazyInitializer = LazyInitializer::new(&|| async move { 3 });

#[used]
static INIT4: LazyInitializer = LazyInitializer::new(&|| async move { 42 });

#[tokio::main]
async fn main() {
    initialize_all().await;
}
