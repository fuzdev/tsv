// CSS selector formatting
//
// Handles formatting of:
// - Selector lists (comma-separated)
// - Complex selectors (with combinators)
// - Relative selectors (simple selector chains)
// - Simple selectors (type, class, id, pseudo-class, pseudo-element, etc.)

use crate::ast::internal;
use crate::formatter::Formatter;

impl Formatter {
    /// Format a selector list (comma-separated complex selectors)
    pub(super) fn format_selector_list(&mut self, list: &internal::SelectorList) {
        for (i, complex) in list.selectors.iter().enumerate() {
            if i > 0 {
                self.write(", ");
            }
            self.format_complex_selector(complex);
        }
    }

    /// Format a complex selector (relative selectors with combinators)
    pub(super) fn format_complex_selector(&mut self, complex: &internal::ComplexSelector) {
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
            self.format_relative_selector(relative);
        }
    }

    /// Format a relative selector (simple selectors without combinator)
    pub(super) fn format_relative_selector(&mut self, relative: &internal::RelativeSelector) {
        for simple in &relative.selectors {
            self.format_simple_selector(simple);
        }
    }

    /// Format a simple selector
    pub(super) fn format_simple_selector(&mut self, simple: &internal::SimpleSelector) {
        match simple {
            internal::SimpleSelector::Type {
                namespace, name, ..
            } => {
                if let Some(ns) = namespace {
                    self.write(ns);
                    self.write("|");
                }
                self.write(name);
            }
            internal::SimpleSelector::Universal { namespace, .. } => {
                if let Some(ns) = namespace {
                    self.write(ns);
                    self.write("|");
                }
                self.write("*");
            }
            internal::SimpleSelector::Class { name, .. } => {
                self.write(".");
                self.write(name);
            }
            internal::SimpleSelector::Id { name, .. } => {
                self.write("#");
                self.write(name);
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
            internal::SimpleSelector::PseudoClass { name, raw_args, .. } => {
                self.write(":");
                self.write(name);
                if let Some(args) = raw_args {
                    self.write("(");
                    self.write(args);
                    self.write(")");
                }
            }
            internal::SimpleSelector::PseudoElement { name, raw_args, .. } => {
                self.write("::");
                self.write(name);
                if let Some(args) = raw_args {
                    self.write("(");
                    self.write(args);
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
}
