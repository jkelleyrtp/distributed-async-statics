use std::{
    any::Any,
    collections::HashMap,
    fmt::{Debug, Display},
    marker::PhantomData,
    panic::Location,
    pin::Pin,
    ptr::addr_of,
    sync::{Arc, LazyLock, Mutex, atomic::AtomicBool},
};

pub struct Lazy<M> {
    static_entry: extern "Rust" fn() -> PinnedAny,
    size: usize,
    caller: &'static Location<'static>,
    us: once_cell::sync::OnceCell<M>,
    _marker: PhantomData<M>,
}

impl<M: 'static + Send + Sync> Lazy<M> {
    #[track_caller]
    pub const fn new<G: FnOnce() -> F + Copy + 'static, F: Future<Output = M> + 'static>(
        f: G,
    ) -> Self {
        let caller = Location::caller();

        let size = size_of_val(&std::hint::black_box(f));
        if size != 0 {
            panic!("Closure passed to Lazy::new must be zero-sized (no captures or references)!");
        }

        Lazy {
            static_entry: __lazy_static_entry::<G, F, M>,
            caller,
            size,
            _marker: PhantomData,
            us: once_cell::sync::OnceCell::new(),
        }
    }

    pub fn get(&self) -> &M {
        // Fast path if already initialized
        if let Some(value) = self.us.get() {
            return value;
        }

        // Otherwise, we need to initialize it
        // Force a read of size and static_entry to prevent optimizations
        unsafe { std::ptr::read_volatile(&self.size) };
        unsafe { std::ptr::read_volatile(&self.static_entry) };

        let initializer = INITIALIZED_MAP
            .lock()
            .unwrap()
            .remove(&(self.static_entry as usize))
            .unwrap();

        let initializer = *initializer.downcast::<M>().unwrap();

        if self.us.set(initializer).is_err() {
            panic!(
                "Failed to set the value for LazyInitializer at {}",
                self.caller
            );
        }

        self.us.get().unwrap()
    }
}

impl<T: 'static + Send + Sync> std::ops::Deref for Lazy<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.get()
    }
}

impl<T: 'static + Send + Sync + Debug> std::fmt::Debug for Lazy<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if f.alternate() {
            let mut debug_struct = f.debug_struct("Lazy");
            debug_struct.field("caller", &self.caller);
            debug_struct.field("size", &self.size);
            debug_struct.field("static_entry", &(self.static_entry as *const fn() as usize));
            debug_struct.field("initialized", &self.us.get().is_some());
            if let Some(value) = self.us.get() {
                debug_struct.field("value", value);
            }
            return debug_struct.finish();
        }

        write!(f, "{:?}", self.get())
    }
}

impl<T: 'static + Send + Sync + Display> std::fmt::Display for Lazy<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.get())
    }
}

static INITIALIZED_MAP: LazyLock<Arc<Mutex<HashMap<usize, Box<dyn Any + Send + Sync>>>>> =
    LazyLock::new(|| Arc::new(Mutex::new(HashMap::new())));

type PinnedAny = Pin<Box<dyn Future<Output = Box<dyn Any + Send + Sync>>>>;

/// Initialize all the lazy statics in the program by calling their initializers
/// This function is safe to call multiple times, but only the first call will do anything.
///
/// It must be called before any Lazy static is accessed, otherwise the program will panic.
pub async fn initialize_all() {
    // only run once....
    static ONCE: AtomicBool = AtomicBool::new(false);
    if ONCE.swap(true, std::sync::atomic::Ordering::SeqCst) {
        return;
    }

    /// Get the size of a function in the section by using a known function and measuring the section
    /// The initializer is passed in here so we can get proper types for the generics
    async fn get_size_of_fn<T: 'static + FnOnce() -> G, G: Future<Output = i32> + 'static>(
        initializer_fn: T,
    ) -> usize {
        let res = __known_fn_size::<T, G, i32>().await;
        assert_eq!(res.as_ref().type_id(), std::any::TypeId::of::<i32>());
        unsafe { std::ptr::read_volatile(&initializer_fn) };

        let start = addr_of!(ONE_SECTION_START) as usize;
        let end = addr_of!(ONE_SECTION_END) as usize;

        end - start
    }

    /// A dummy initializer with the right signature to get the size of a function in the section
    #[inline(never)]
    extern "Rust" fn dummy_initializer() -> Pin<Box<dyn Future<Output = i32>>> {
        Box::pin(async move { std::hint::black_box(42) })
    }

    // Initialize the map with the results of all the initializers in the section. We'll swap it later
    let mut map = HashMap::new();

    // We pass in the dummy_initializer function to get its size as a function (in assembly instructions...)
    let width = get_size_of_fn(dummy_initializer).await;

    // now that we have the width of a known function, we can use that to iterate over the actual section
    let start = addr_of!(SECTION_START) as usize;
    let end = addr_of!(SECTION_END) as usize;
    let mut current = start;

    // Walk the __TEXT section, calling each function in turn and storing the result in the map
    while current + width <= end {
        let user_func: extern "Rust" fn() -> PinnedAny =
            unsafe { std::mem::transmute_copy(&current) };
        let user_future = user_func();
        let result = user_future.await;
        map.insert(current, result);
        current += width;
    }

    *INITIALIZED_MAP.lock().unwrap() = map;
}

unsafe extern "Rust" {
    #[link_name = "\x01section$start$__TEXT$__lazy_async"]
    static SECTION_START: u8;

    #[link_name = "\x01section$end$__TEXT$__lazy_async"]
    static SECTION_END: u8;

    #[link_name = "\x01section$start$__TEXT$__lazy_known_fn"]
    static ONE_SECTION_START: u8;

    #[link_name = "\x01section$end$__TEXT$__lazy_known_fn"]
    static ONE_SECTION_END: u8;
}

/// The size of this codegen is very intended to be fixed no matter the size of T since there's no args.
#[inline(never)]
#[unsafe(link_section = "__TEXT,__lazy_async")]
extern "Rust" fn __lazy_static_entry<T, G, M>() -> PinnedAny
where
    T: 'static + FnOnce() -> G,
    G: Future<Output = M> + 'static,
    M: 'static + Send + Sync,
{
    __fixed_size_lazy_static_initializer::<T, G, M>()
}

/// The size of this codegen is very intended to be fixed no matter the size of T since there's no args.
#[inline(never)]
#[unsafe(link_section = "__TEXT,__lazy_known_fn")]
extern "Rust" fn __known_fn_size<T, G, M>() -> PinnedAny
where
    T: 'static + FnOnce() -> G,
    G: Future<Output = M> + 'static,
    M: 'static + Send + Sync,
{
    __fixed_size_lazy_static_initializer::<T, G, M>()
}

#[inline(never)]
extern "Rust" fn __fixed_size_lazy_static_initializer<T, G, M>() -> PinnedAny
where
    T: 'static + FnOnce() -> G,
    G: Future<Output = M> + 'static,
    M: 'static + Send + Sync,
{
    let size_of_t = std::mem::size_of::<T>();

    assert_eq!(size_of_t, 0, "Somehow you got a non-zero sized closure!");

    // Run the user's future and box the result so it can be downcasted later.
    Box::pin(async move {
        let terrible_closure: T = unsafe { std::mem::zeroed() };

        // we're going to to some terrible stuff here.
        let result = terrible_closure().await;

        Box::new(result) as Box<dyn Any + Send + Sync>
    })
}
