use std::{collections::HashMap, marker::PhantomData, panic::Location, pin::Pin, ptr::addr_of};

// #[inline(never)]
// #[unsafe(link_section = "__TEXT,__custom_static")]
// pub extern "Rust" fn ___initialize_all_static_entries() {
//     let our_ptr = ___initialize_all_static_entries as *const ();
//     let bytes_of_us = our_ptr as usize;
// }

// static INITIALIZED: HashMap<*const (), i32> = HashMap::new();

pub async fn initialize_all() {
    async fn get_size_of_fn<T: 'static + Fn() -> G, G: Future<Output = i32> + 'static>(
        t: T,
    ) -> usize {
        let res = known_fn_size::<T, G>().await;
        println!("Result of known_fn_size: {}", res);

        unsafe { std::ptr::read_volatile(&t) };

        let start = addr_of!(ONE_SECTION_START) as usize;
        let end = addr_of!(ONE_SECTION_END) as usize;

        end - start
    }

    // We pass in the dummy_initializer function to get its size as a function (in assembly instructions...)
    let width = get_size_of_fn(dummy_initializer).await;

    println!("Size of known_fn_size: {}", width);

    // now that we have the width of a known function, we can use that to iterate over the actual section
    let start = addr_of!(SECTION_START) as usize;
    let end = addr_of!(SECTION_END) as usize;
    let mut current = start;

    let mut distributed_inialize_count = 0;

    while current + width <= end {
        let func: extern "Rust" fn() -> Pin<Box<dyn Future<Output = i32>>> =
            unsafe { std::mem::transmute_copy(&current) };
        let fut = func();
        let res = fut.await;
        distributed_inialize_count += res;

        current += width;
    }

    println!(
        "Distributed initialize count: {}",
        distributed_inialize_count
    );
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
        println!("Dummy initializer called!");
        42
    })
}

/// The size of this codegen is very intended to be fixed no matter the size of T since there's no args.
#[inline(never)]
#[unsafe(link_section = "__TEXT,__custom_static")]
pub extern "Rust" fn write_static_entry_for<
    T: 'static + Fn() -> G,
    G: Future<Output = i32> + 'static,
>() -> Pin<Box<dyn Future<Output = i32>>> {
    fixed_size_inner::<T, G>()
}

/// The size of this codegen is very intended to be fixed no matter the size of T since there's no args.
#[inline(never)]
#[unsafe(link_section = "__TEXT,__one_entry")]
pub extern "Rust" fn known_fn_size<T: 'static + Fn() -> G, G: Future<Output = i32> + 'static>()
-> Pin<Box<dyn Future<Output = i32>>> {
    fixed_size_inner::<T, G>()
}

#[inline(never)]
extern "Rust" fn fixed_size_inner<T: 'static + Fn() -> G, G: Future<Output = i32> + 'static>()
-> Pin<Box<dyn Future<Output = i32>>> {
    let size_of_t = std::mem::size_of::<T>();

    println!(
        "initializing {} with size {}",
        std::any::type_name::<T>(),
        size_of_t
    );

    assert_eq!(size_of_t, 0, "Somehow you got a non-zero sized closure!");

    let terrible_closure: T = unsafe { std::mem::zeroed() };

    // we're going to to some terrible stuff here.
    let res = terrible_closure();

    println!("Static entry for type: {}!", std::any::type_name::<T>(),);

    Box::pin(res) as Pin<Box<dyn Future<Output = i32>>>
}

pub struct LazyInitializer {
    static_entry: extern "Rust" fn() -> Pin<Box<dyn Future<Output = i32>>>,
    ptr: *const (),
    caller: &'static Location<'static>,
}

unsafe impl Send for LazyInitializer {}
unsafe impl Sync for LazyInitializer {}

impl LazyInitializer {
    pub const fn new<G: Fn() -> F + Copy + 'static, F: Future<Output = i32> + 'static>(
        f: &'static G,
    ) -> Self {
        let caller = Location::caller();
        LazyInitializer {
            static_entry: write_static_entry_for::<G, F>,
            ptr: f as *const G as *const (),
            caller,
        }
    }

    pub async fn initialize(&self) {
        unsafe { std::ptr::read_volatile(&self.ptr) };
        unsafe { std::ptr::read_volatile(&self.static_entry) };
    }
}
