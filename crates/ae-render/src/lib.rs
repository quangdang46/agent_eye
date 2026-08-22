pub mod braille;
pub mod budget;
pub mod charset;
pub mod config;
pub mod render;
pub mod sampling;

pub use braille::{render_braille, BrailleConfig};
pub use budget::{adaptive_slices, allocate, slice_transform, BudgetSlice};

pub use charset::{presets, Charset, MAX_CHARSET_LEN};
pub use config::{ColorMode, RenderConfig, RendererType, DEFAULT_RENDER_WIDTH};
pub use render::{render, render_ascii, render_blocks, RenderedGrid};
pub use sampling::{sample_blocks, Block, DEFAULT_ASPECT_RATIO};

pub const CRATE_NAME: &str = env!("CARGO_PKG_NAME");

#[cfg(test)]
mod tests {
    #[test]
    fn crate_name() {
        assert_eq!(super::CRATE_NAME, "ae-render");
    }
}
