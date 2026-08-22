pub mod charset;

pub use charset::{presets, Charset, MAX_CHARSET_LEN};

pub const CRATE_NAME: &str = env!("CARGO_PKG_NAME");

#[cfg(test)]
mod tests {
    #[test]
    fn crate_name() {
        assert_eq!(super::CRATE_NAME, "ae-render");
    }
}
