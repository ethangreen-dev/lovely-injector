echo '#![allow(non_snake_case, non_camel_case_types, non_upper_case_globals)]' > src/ffi.rs
bindgen bind.h >> src/ffi.rs