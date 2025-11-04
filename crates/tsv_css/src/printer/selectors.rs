// CSS selector formatting
//
// Handles formatting of:
// - Selector lists (comma-separated)
// - Complex selectors (with combinators)
// - Relative selectors (simple selector chains)
// - Simple selectors (type, class, id, pseudo-class, pseudo-element, etc.)

use super::Printer;
use crate::ast::internal;

impl<'a> Printer<'a> {
    /// Format a selector list (comma-separated complex selectors)
    pub(super) fn print_selector_list(&mut self, list: &internal::SelectorList) {
        for (i, complex) in list.selectors.iter().enumerate() {
            if i > 0 {
                self.write(", ");
            }
            self.print_complex_selector(complex);
        }
    }

    /// Format a complex selector (relative selectors with combinators)
    pub(super) fn print_complex_selector(&mut self, complex: &internal::ComplexSelector) {
        for (i, relative) in complex.children.iter().enumerate() {
            if i > 0 {
                // Add combinator spacing
                if let Some(combinator) = relative.combinator {
                    match combinator {
                        internal::Combinator::Descendant => self.write(" "),
                        internal::Combinator::Child => self.write(" > "),
                        internal::Combinator::NextSibling => self.write(" + "),
                        internal::Combinator::SubsequentSibling => self.write(" ~ "),
                        internal::Combinator::Column => self.write(" || "),
                    }
                }
            }
            self.print_relative_selector(relative);
        }
    }

    /// Format a relative selector (simple selectors without combinator)
    pub(super) fn print_relative_selector(&mut self, relative: &internal::RelativeSelector) {
        for simple in &relative.selectors {
            self.print_simple_selector(simple);
        }
    }

    /// Format a simple selector
    pub(super) fn print_simple_selector(&mut self, simple: &internal::SimpleSelector) {
        match simple {
            internal::SimpleSelector::Type {
                namespace,
                name: _,
                span,
            } => {
                // SVELTE QUIRK: Extract raw from source to preserve escape sequences
                // Type selectors can have escapes (uncommon but valid)
                // Example: `d\69v` stays as `d\69v`, not `div`
                // Example: `\30span` stays as `\30span`, not `0span`
                //
                // See:
                // - docs/SVELTE_COMPATIBILITY.md (CSS Quirks section)
                // - tests/fixtures/css/escapes/type_selector_escaped (demonstrates this behavior)
                // - Svelte source: node_modules/svelte/src/compiler/phases/1-parse/read/style.js:575-611
                if let Some(ns) = namespace {
                    self.write(ns);
                    self.write("|");
                }
                let raw = &self.source[span.start as usize..span.end as usize];
                self.write(raw);
            }
            internal::SimpleSelector::Universal { namespace, .. } => {
                if let Some(ns) = namespace {
                    self.write(ns);
                    self.write("|");
                }
                self.write("*");
            }
            internal::SimpleSelector::Class { name: _, span } => {
                // SVELTE QUIRK: Extract raw from source to preserve escape sequences
                // Svelte does NOT decode escape sequences in CSS identifiers (selectors, property names)
                // Example: `.cl\41ss` stays as `.cl\41ss`, not `.clAss`
                //
                // See:
                // - docs/SVELTE_COMPATIBILITY.md (CSS Quirks section)
                // - tests/fixtures/css/escapes/unicode_in_identifiers (demonstrates this behavior)
                // - Svelte source: node_modules/svelte/src/compiler/phases/1-parse/read/style.js:575-611
                let raw = &self.source[span.start as usize..span.end as usize];
                self.write(raw); // Includes the '.' prefix
            }
            internal::SimpleSelector::Id { name: _, span } => {
                // SVELTE QUIRK: Extract raw from source to preserve escape sequences
                // Same behavior as class selectors - identifiers preserve raw escapes
                // Example: `#\1F4A9-id` stays as `#\1F4A9-id` (escape not decoded)
                //
                // See docs/SVELTE_COMPATIBILITY.md and tests/fixtures/css/escapes/unicode_in_identifiers
                let raw = &self.source[span.start as usize..span.end as usize];
                self.write(raw); // Includes the '#' prefix
            }
            internal::SimpleSelector::Attribute {
                namespace,
                name,
                matcher,
                value,
                flags,
                ..
            } => {
                self.write("[");
                if let Some(ns) = namespace {
                    self.write(ns);
                    self.write("|");
                }
                self.write(name);
                if let Some(m) = matcher {
                    let op = match m {
                        internal::AttributeMatcher::Exact => "=",
                        internal::AttributeMatcher::Contains => "~=",
                        internal::AttributeMatcher::DashMatch => "|=",
                        internal::AttributeMatcher::Prefix => "^=",
                        internal::AttributeMatcher::Suffix => "$=",
                        internal::AttributeMatcher::Substring => "*=",
                    };
                    self.write(op);
                    if let Some(v) = value {
                        // TODO: Determine if value needs quotes
                        self.write("'");
                        self.write(v);
                        self.write("'");
                    }
                }
                if let Some(f) = flags {
                    self.write(" ");
                    self.write(f);
                }
                self.write("]");
            }
            internal::SimpleSelector::PseudoClass { name, args, .. } => {
                self.write(":");
                self.write(name);
                if let Some(args) = args {
                    self.write("(");
                    self.print_pseudo_class_args(args);
                    self.write(")");
                }
            }
            internal::SimpleSelector::PseudoElement { name, args, .. } => {
                self.write("::");
                self.write(name);
                if let Some(args) = args {
                    self.write("(");
                    self.print_pseudo_class_args(args);
                    self.write(")");
                }
            }
            internal::SimpleSelector::Nesting { .. } => {
                self.write("&");
            }
            internal::SimpleSelector::Percentage { value, .. } => {
                self.write(&format!("{}%", value));
            }
        }
    }

    /// Format pseudo-class/pseudo-element arguments
    fn print_pseudo_class_args(&mut self, args: &internal::PseudoClassArgs) {
        match args {
            internal::PseudoClassArgs::Nth { value, .. } => {
                self.write(value);
            }
        }
    }
}
