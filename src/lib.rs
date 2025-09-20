use std::{
    any::Any,
    collections::HashMap,
    marker::PhantomData,
    panic::Location,
    pin::Pin,
    ptr::addr_of,
    sync::{Arc, LazyLock, Mutex},
};

// #[inline(never)]
// #[unsafe(link_section = "__TEXT,__custom_static")]
// pub extern "Rust" fn ___initialize_all_static_entries() {
//     let our_ptr = ___initialize_all_static_entries as *const ();
//     let bytes_of_us = our_ptr as usize;
// }

static INITIALIZED: LazyLock<Arc<Mutex<HashMap<usize, Box<dyn Any + Send + Sync>>>>> =
    LazyLock::new(|| Arc::new(Mutex::new(HashMap::new())));

pub async fn initialize_all() {
    async fn get_size_of_fn<T: 'static + Fn() -> G, G: Future<Output = i32> + 'static>(
        t: T,
    ) -> usize {
        let res = known_fn_size::<T, G, i32>().await;
        assert_eq!(res.as_ref().type_id(), std::any::TypeId::of::<i32>());

        unsafe { std::ptr::read_volatile(&t) };

        let start = addr_of!(ONE_SECTION_START) as usize;
        let end = addr_of!(ONE_SECTION_END) as usize;

        end - start
    }

    // We pass in the dummy_initializer function to get its size as a function (in assembly instructions...)
    let width = get_size_of_fn(dummy_initializer).await;

    // now that we have the width of a known function, we can use that to iterate over the actual section
    let start = addr_of!(SECTION_START) as usize;
    let end = addr_of!(SECTION_END) as usize;
    let mut current = start;

    while current + width <= end {
        let func: extern "Rust" fn() -> Pin<Box<dyn Future<Output = Box<dyn Any + Send + Sync>>>> =
            unsafe { std::mem::transmute_copy(&current) };
        let fut = func();
        let res = fut.await;
        INITIALIZED.lock().unwrap().insert(current, res);

        current += width;
    }
}

unsafe extern "Rust" {
    #[link_name = "\x01section$start$__TEXT$__custom_static"]
    static SECTION_START: u8;

    #[link_name = "\x01section$end$__TEXT$__custom_static"]
    static SECTION_END: u8;

    #[link_name = "\x01section$start$__TEXT$__one_entry"]
    static ONE_SECTION_START: u8;

    #[link_name = "\x01section$end$__TEXT$__one_entry"]
    static ONE_SECTION_END: u8;
}

#[inline(never)]
extern "Rust" fn dummy_initializer() -> Pin<Box<dyn Future<Output = i32>>> {
    Box::pin(async move {
        std::hint::black_box(());
        42
    })
}

/// The size of this codegen is very intended to be fixed no matter the size of T since there's no args.
#[inline(never)]
#[unsafe(link_section = "__TEXT,__custom_static")]
pub extern "Rust" fn write_static_entry_for<
    T: 'static + Fn() -> G,
    G: Future<Output = M> + 'static,
    M: 'static + Send + Sync,
>() -> Pin<Box<dyn Future<Output = Box<dyn Any + Send + Sync>>>> {
    fixed_size_inner::<T, G, M>()
}

/// The size of this codegen is very intended to be fixed no matter the size of T since there's no args.
#[inline(never)]
#[unsafe(link_section = "__TEXT,__one_entry")]
pub extern "Rust" fn known_fn_size<
    T: 'static + Fn() -> G,
    G: Future<Output = M> + 'static,
    M: 'static + Send + Sync,
>() -> Pin<Box<dyn Future<Output = Box<dyn Any + Send + Sync>>>> {
    fixed_size_inner::<T, G, M>()
}

#[inline(never)]
extern "Rust" fn fixed_size_inner<
    T: 'static + Fn() -> G,
    G: Future<Output = M> + 'static,
    M: 'static + Send + Sync,
>() -> Pin<Box<dyn Future<Output = Box<dyn Any + Send + Sync>>>> {
    let size_of_t = std::mem::size_of::<T>();

    assert_eq!(size_of_t, 0, "Somehow you got a non-zero sized closure!");

    let terrible_closure: T = unsafe { std::mem::zeroed() };

    // we're going to to some terrible stuff here.
    let res = terrible_closure();

    Box::pin(async move {
        let res = res.await;
        Box::new(res) as Box<dyn Any + Send + Sync>
    }) as Pin<Box<dyn Future<Output = Box<dyn Any + Send + Sync>>>>
}

pub struct LazyInitializer<M> {
    static_entry: extern "Rust" fn() -> Pin<Box<dyn Future<Output = Box<dyn Any + Send + Sync>>>>,
    ptr: *const (),
    caller: &'static Location<'static>,
    us: once_cell::sync::OnceCell<M>,
    _marker: PhantomData<M>,
}

unsafe impl<M> Send for LazyInitializer<M> {}
unsafe impl<M> Sync for LazyInitializer<M> {}

impl<M: 'static + Send + Sync> LazyInitializer<M> {
    pub const fn new<G: Fn() -> F + Copy + 'static, F: Future<Output = M> + 'static>(
        f: &'static G,
    ) -> Self {
        let caller = Location::caller();
        LazyInitializer {
            static_entry: write_static_entry_for::<G, F, M>,
            ptr: f as *const G as *const (),
            caller,
            _marker: PhantomData,
            us: once_cell::sync::OnceCell::new(),
        }
    }

    pub fn get(&self) -> &M {
        if self.us.get().is_none() {
            unsafe { std::ptr::read_volatile(&self.ptr) };
            unsafe { std::ptr::read_volatile(&self.static_entry) };

            let initializer = INITIALIZED
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
        }

        self.us.get().unwrap()
    }
}

impl<T: 'static + Send + Sync> std::ops::Deref for LazyInitializer<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.get()
    }
}
