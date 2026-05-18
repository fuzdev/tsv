//! Internal renderer width configuration.
//!
//! Production callers go through the public renderer entry points
//! ([`super::arena_render::arena_print_doc`] etc.), which always use
//! [`RenderConfig::default()`] — i.e. [`crate::PRINT_WIDTH`],
//! [`crate::TAB_WIDTH`], [`crate::INDENT`]. These values are hardcoded for
//! the formatter and never overridden by users.
//!
//! This struct exists so the doc-builder unit tests can exercise the
//! algorithm with smaller widths (e.g. `print_width: 10`) without bloating
//! test inputs. It is `pub(crate)` and intentionally not part of the public
//! API.

use crate::{INDENT, PRINT_WIDTH, TAB_WIDTH};

#[derive(Debug, Clone, Copy)]
pub(crate) struct RenderConfig {
    pub print_width: usize,
    pub tab_width: usize,
    pub indent: &'static str,
}

impl Default for RenderConfig {
    fn default() -> Self {
        Self {
            print_width: PRINT_WIDTH,
            tab_width: TAB_WIDTH,
            indent: INDENT,
        }
    }
}
