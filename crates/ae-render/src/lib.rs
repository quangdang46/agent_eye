pub mod charset;
pub mod sampling;

pub use charset::{presets, Charset, MAX_CHARSET_LEN};
pub use sampling::{sample_blocks, Block, DEFAULT_ASPECT_RATIO};

pub const CRATE_NAME: &str = env!("CARGO_PKG_NAME");

#[cfg(test)]
mod tests {
    #[test]
    fn crate_name() {
        assert_eq!(super::CRATE_NAME, "ae-render");
    }
}
