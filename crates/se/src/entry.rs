//! The six entry points, as macros.
//!
//! Every one of these expands to a `#[no_mangle] extern "C"` function the host
//! looks up by name. Writing them by hand is possible and never necessary.

/// Stamp the ABI version into the image. The host reads this first and refuses
/// the module outright if the major disagrees.
#[macro_export]
macro_rules! abi_version {
    () => {
        #[no_mangle]
        pub extern "C" fn se_abi_version() -> $crate::AbiVersion {
            $crate::AbiVersion::CURRENT
        }
    };
}

/// `bundle/data.rs` — the component half of the contract.
///
/// ```ignore
/// se::layouts!(Transform, Body);
/// ```
#[macro_export]
macro_rules! layouts {
    ($($t:ty),* $(,)?) => {
        /// # Safety
        /// Called by the host with a sink it owns.
        #[no_mangle]
        pub unsafe extern "C" fn se_register_layouts(sink: *mut $crate::LayoutSink) {
            let sink = &mut *sink;
            $( sink.push(&<$t as $crate::Schema>::layout()); )*
        }
    };
}

/// `bundle/buffer.rs` — the render-target half of the contract.
///
/// ```ignore
/// se::buffers!(
///     se::BufferDesc::screen("scene", se::Format::Rgba8Unorm),
///     se::BufferDesc::screen("depth", se::Format::Depth32Float),
/// );
/// ```
#[macro_export]
macro_rules! buffers {
    ($($b:expr),* $(,)?) => {
        /// # Safety
        /// Called by the host with a sink it owns.
        #[no_mangle]
        pub unsafe extern "C" fn se_register_buffers(sink: *mut $crate::BufferSink) {
            let sink = &mut *sink;
            $( sink.push(&$b); )*
        }
    };
}

/// `asset/*.so` — name to bytes, and nothing else. Content never writes data.
///
/// ```ignore
/// se::assets! {
///     "bunny.obj" => include_bytes!("bunny/model.obj"),
/// }
/// ```
#[macro_export]
macro_rules! assets {
    ($($name:literal => $bytes:expr),* $(,)?) => {
        $crate::abi_version!();

        /// # Safety
        /// Called by the host with a sink it owns.
        #[no_mangle]
        pub unsafe extern "C" fn se_register_assets(sink: *mut $crate::AssetSink) {
            let sink = &mut *sink;
            $( sink.push(&$crate::AssetDesc::new($name, $bytes)); )*
        }
    };
}

/// `render/*.so` — nodes and edges.
///
/// ```ignore
/// se::graph!(|g| g
///     .present("scene")
///     .pass("bodies", |p| p
///         .shader(format!("{}{}", wgsl::MESH_VS, shade::FS))
///         .color(&["scene"])
///         .depth("depth")
///         .uniform_of("Camera")
///         .instanced("Transform", "model.obj")));
/// ```
#[macro_export]
macro_rules! graph {
    ($name:literal, $build:expr) => {
        $crate::abi_version!();

        /// # Safety
        /// Called by the host with a sink it owns.
        #[no_mangle]
        pub unsafe extern "C" fn se_register_graph(sink: *mut $crate::GraphSink) {
            let build: fn($crate::GraphBuilder) -> $crate::GraphBuilder = $build;
            build($crate::GraphBuilder::new($name)).finish(&mut *sink);
        }
    };
}

/// `game/*.so` — control. Slots say which module fills which position; `tick`
/// is the only place in the whole engine that knows what time it is.
///
/// ```ignore
/// se::control! {
///     name: "game1",
///     slots: [se::SlotBind::new(se::Slot::Render, "graph1")],
///     start: start,
///     tick: tick,
/// }
/// ```
#[macro_export]
macro_rules! control {
    (
        name: $name:literal,
        slots: [$($s:expr),* $(,)?],
        $(start: $start:path,)?
        tick: $tick:path
        $(, stop: $stop:path)?
        $(,)?
    ) => {
        $crate::abi_version!();

        static __SE_SLOTS: &[$crate::SlotBind] = &[$($s),*];

        /// # Safety
        /// Called by the host with a sink it owns.
        #[no_mangle]
        pub unsafe extern "C" fn se_register_control(sink: *mut $crate::ControlSink) {
            unsafe extern "C" fn __se_tick(ctl: *mut $crate::Ctl, f: *const $crate::Frame) {
                $tick(&mut *ctl, &*f)
            }
            $(
                unsafe extern "C" fn __se_start(ctl: *mut $crate::Ctl) {
                    $start(&mut *ctl)
                }
            )?
            $(
                unsafe extern "C" fn __se_stop(ctl: *mut $crate::Ctl) {
                    $stop(&mut *ctl)
                }
            )?

            #[allow(unused_mut)]
            let mut start: Option<unsafe extern "C" fn(*mut $crate::Ctl)> = None;
            $( { let _ = stringify!($start); start = Some(__se_start); } )?
            #[allow(unused_mut)]
            let mut stop: Option<unsafe extern "C" fn(*mut $crate::Ctl)> = None;
            $( { let _ = stringify!($stop); stop = Some(__se_stop); } )?

            let c = $crate::ControlSpec {
                name: $crate::Str::new($name),
                slots: $crate::Slice::new(__SE_SLOTS),
                start,
                tick: __se_tick,
                stop,
            };
            (&mut *sink).push(&c);
        }
    };
}
