# distributed initialization - or lazy constructors

Have you ever wanted an async once-cell?

But didn't want to call `.await` everywhere?

Introducing: distributed async initializers!

Simply, call `initialize()` and all `Lazy<T>` initializers will initialize! Deref directly to the `Lazy` value, no `.await` required!

```rust
static CUTE_DOG: Lazy<serde_json::Value> = Lazy::new(|| async {
    reqwest::get("https://dog.ceo/api/breeds/image/random")
        .await
        .unwrap()
        .json::<serde_json::Value>()
        .await
        .unwrap()
});

static DATABASE: Lazy<q>

```
