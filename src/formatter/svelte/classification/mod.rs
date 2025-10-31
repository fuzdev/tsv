// Element and content classification for Svelte formatting
//
// Provides utilities to classify elements by their rendering characteristics
// and query properties of template content.
//
// Organization:
// - element.rs: Element type checks (inline, block, void)
// - whitespace.rs: Whitespace preservation rules
// - content.rs: Fragment content analysis

pub mod content;
pub mod element;
