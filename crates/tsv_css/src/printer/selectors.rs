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
        // Check if source contains comments (/* ... */)
        let source_text = list.span.extract(self.source);
        let has_comments = source_text.contains("/*");

        if has_comments {
            // Extract from source and normalize whitespace around comments
            // Replace patterns like ",/*" with ", /*" and "*/" with "*/ "
            // TODO: Refactor to use self.normalize_comment_spacing() for consistency
            // Current implementation uses manual .replace() calls (6 lines of duplication)
            // Would need testing against selector fixtures with comments to ensure identical output
            let normalized = source_text
                .replace(",/*", ", /*")
                .replace("*/.", "*/ .")
                .replace(",  /*", ", /*") // Reduce multiple spaces after comma
                .replace("*/  .", "*/ ."); // Reduce multiple spaces after comment
            self.write(&normalized);
        } else {
            // No comments - use AST formatting
            for (i, complex) in list.selectors.iter().enumerate() {
                if i > 0 {
                    self.write(", ");
                }
                self.print_complex_selector(complex);
            }
        }
    }

    /// Format a complex selector (relative selectors with combinators)
    pub(super) fn print_complex_selector(&mut self, complex: &internal::ComplexSelector) {
        for (i, relative) in complex.children.iter().enumerate() {
            // Pass whether this is the first selector (index 0) to determine combinator spacing
            let is_first = i == 0;
            self.print_relative_selector_internal(relative, is_first);
        }
    }

    /// Internal helper to format a relative selector with context
    fn print_relative_selector_internal(
        &mut self,
        relative: &internal::RelativeSelector,
        is_first_in_complex: bool,
    ) {
        // Print combinator if present
        if let Some(combinator) = &relative.combinator {
            // Leading combinator: first selector in a complex selector with a combinator
            // Example: :has(> img) - the > is leading (no space before)
            // Between combinator: subsequent selectors in a complex selector
            // Example: div > span - the > is between (space before and after)
            let is_leading = is_first_in_complex;

            match combinator {
                internal::Combinator::Descendant => {
                    // Descendant combinator is just a space (no extra space needed)
                    self.write(" ");
                }
                internal::Combinator::Child => {
                    if is_leading {
                        self.write("> "); // Leading: no space before
                    } else {
                        self.write(" > "); // Between selectors: space before and after
                    }
                }
                internal::Combinator::NextSibling => {
                    if is_leading {
                        self.write("+ ");
                    } else {
                        self.write(" + ");
                    }
                }
                internal::Combinator::SubsequentSibling => {
                    if is_leading {
                        self.write("~ ");
                    } else {
                        self.write(" ~ ");
                    }
                }
                internal::Combinator::Column => {
                    if is_leading {
                        self.write("|| ");
                    } else {
                        self.write(" || ");
                    }
                }
            }
        }

        for simple in &relative.selectors {
            self.print_simple_selector(simple);
        }
    }

    /// Format a simple selector
    pub(super) fn print_simple_selector(&mut self, simple: &internal::SimpleSelector) {
        match simple {
            internal::SimpleSelector::Type {
                namespace: _, // Namespace already included in span/source
                name: _,
                span,
            } => {
                // SVELTE QUIRK: Extract raw from source to preserve escape sequences
                // Type selectors can have escapes (uncommon but valid)
                // Example: `d\69v` stays as `d\69v`, not `div`
                // Example: `\30span` stays as `\30span`, not `0span`
                //
                // Namespace prefixes are also preserved in the raw source:
                // Example: `svg|rect` is extracted as-is from the source
                //
                // See:
                // - docs/SVELTE_COMPATIBILITY.md (CSS Quirks section)
                // - tests/fixtures/css/escapes/type_selector_escaped (demonstrates this behavior)
                // - Svelte source: node_modules/svelte/src/compiler/phases/1-parse/read/style.js:575-611
                let raw = span.extract(self.source);
                self.write(raw);
            }
            internal::SimpleSelector::Universal { namespace, .. } => {
                // Universal namespace prefix needs explicit handling since the span
                // may not include the * (when parsing *|div, the span is for *|div)
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
                let raw = span.extract(self.source);
                self.write(raw); // Includes the '.' prefix
            }
            internal::SimpleSelector::Id { name: _, span } => {
                // SVELTE QUIRK: Extract raw from source to preserve escape sequences
                // Same behavior as class selectors - identifiers preserve raw escapes
                // Example: `#\1F4A9-id` stays as `#\1F4A9-id` (escape not decoded)
                //
                // See docs/SVELTE_COMPATIBILITY.md and tests/fixtures/css/escapes/unicode_in_identifiers
                let raw = span.extract(self.source);
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
            internal::SimpleSelector::Invalid { raw, .. } => {
                // Forgiving selector list - preserve invalid selector as-is
                // Used in :is() and :where() to maintain source fidelity
                // Example: `:is(.a, ., .b)` preserves the `.` even though it's invalid
                self.write(raw);
            }
        }
    }

    /// Normalize An+B notation spacing (better than prettier)
    ///
    /// Per CSS Syntax spec: "Whitespace is valid (and ignored) between any other two tokens"
    /// This means we can normalize for consistency without changing semantics.
    ///
    /// Our normalization (better than prettier):
    /// - `2n+1` → `2n + 1` (always add spaces around +)
    /// - `3n-2` → `3n - 2` (always add spaces around -, unlike prettier)
    /// - `2n  +  1` → `2n + 1` (collapse multiple spaces)
    /// - `  n  ` → `n` (trim outer spaces)
    /// - `odd`, `even`, `3` → unchanged
    ///
    /// Why we normalize minus (unlike prettier):
    /// The spec says whitespace is ignored, so `3n-2` === `3n - 2`.
    /// Consistent spacing improves readability.
    fn normalize_an_plus_b(value: &str) -> String {
        let trimmed = value.trim();

        // Simple cases: keywords and plain numbers (no operators)
        if !trimmed.contains('+') && !trimmed.contains('-') {
            return trimmed.to_string();
        }

        // Normalize spacing around + and - operators
        let mut result = String::with_capacity(trimmed.len() + 4);
        let chars: Vec<char> = trimmed.chars().collect();
        let mut i = 0;

        while i < chars.len() {
            let ch = chars[i];

            // Handle + and - operators: always normalize to ` op `
            if (ch == '+' || ch == '-') && i > 0 {
                // Check if this is an operator (has content before it)
                let trimmed_result = result.trim_end();
                if !trimmed_result.is_empty() {
                    // This is an operator - normalize spacing
                    result.truncate(trimmed_result.len());
                    result.push(' ');
                    result.push(ch);

                    // Skip any spaces after operator and add single space
                    i += 1;
                    while i < chars.len() && chars[i].is_whitespace() {
                        i += 1;
                    }
                    if i < chars.len() {
                        result.push(' ');
                    }
                    continue;
                }
            }

            result.push(ch);
            i += 1;
        }

        result.trim().to_string()
    }

    /// Format pseudo-class/pseudo-element arguments
    fn print_pseudo_class_args(&mut self, args: &internal::PseudoClassArgs) {
        match args {
            internal::PseudoClassArgs::Nth {
                value, of_selector, ..
            } => {
                let normalized = Self::normalize_an_plus_b(value);
                self.write(&normalized);
                // CSS Selectors Level 4: :nth-child(An+B of S)
                if let Some(selectors) = of_selector {
                    self.write(" of ");
                    self.print_selector_list(selectors);
                }
            }
            internal::PseudoClassArgs::SelectorList { selectors, .. } => {
                self.print_selector_list(selectors);
            }
            internal::PseudoClassArgs::Slotted { selectors, .. } => {
                // Format compound selector: sequence of simple selectors (no combinators)
                // Examples: `*`, `div`, `.foo`, `div.foo#bar:hover`
                for selector in selectors {
                    self.print_simple_selector(selector);
                }
            }
            internal::PseudoClassArgs::Part { idents, .. } => {
                // Format part names: space-separated identifiers
                // Examples: `label`, `tab active`, `button primary`
                for (i, ident) in idents.iter().enumerate() {
                    if i > 0 {
                        self.write(" ");
                    }
                    self.write(ident);
                }
            }
            internal::PseudoClassArgs::Identifier { value, .. } => {
                // Format identifier arguments for spec-compliant pseudo-classes/elements
                // Examples: :dir(ltr), :lang(en-US), ::highlight(search-results)
                self.write(value);
            }
        }
    }
}
