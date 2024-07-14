#[cfg(not(debug_assertions))]
#[macro_export]
macro_rules! version {
    () => {
        env!("CARGO_PKG_VERSION")
    };
    ($prefix:expr) => {
        format!("{}{}", $prefix, env!("CARGO_PKG_VERSION"))
    };
}

#[cfg(debug_assertions)]
#[macro_export]
macro_rules! version {
    () => {
        "0.0.0-dev"
    };
    ($prefix:expr) => {
        format!("{}0.0.0-dev", $prefix)
    };
}
