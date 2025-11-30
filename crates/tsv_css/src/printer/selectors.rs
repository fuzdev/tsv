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
    ///
    /// Supports line wrapping for top-level selector lists (in rules).
    /// Nested selector lists (inside :is(), :where(), etc.) are NOT wrapped.
    pub(super) fn print_selector_list(&mut self, list: &internal::SelectorList) {
        self.print_selector_list_internal(list, false);
    }

    /// Format a selector list in nested context (inside pseudo-class arguments or @scope)
    ///
    /// For short lists: prints inline (e.g., `:is(.a, .b)`)
    /// For long lists: wraps each selector on its own line with indentation
    pub(super) fn print_selector_list_nested(&mut self, list: &internal::SelectorList) {
        use tsv_lang::doc;

        if list.selectors.is_empty() {
            return;
        }

        // Check if source contains comments
        let source_text = list.span.extract(self.source);
        if source_text.contains("/*") {
            // Extract from source and normalize whitespace around comments
            let normalized = source_text
                .replace(",/*", ", /*")
                .replace("*/.", "*/ .")
                .replace(",  /*", ", /*")
                .replace("*/  .", "*/ .");
            self.write(&normalized);
            return;
        }

        // Build a doc for the selector list to check if it fits
        let list_doc = self.build_selector_list_doc(list);

        // Check if it fits on one line
        // For nested selector lists, we use a smaller threshold since they're inside parens
        // and need to account for the surrounding context
        let available_width = self.config.print_width.saturating_sub(20); // Conservative threshold
        let fits = doc::fits(&list_doc, available_width, doc::Mode::Flat, &self.config);

        if fits {
            // Print inline
            for (i, complex) in list.selectors.iter().enumerate() {
                if i > 0 {
                    self.write(", ");
                }
                self.print_complex_selector(complex);
            }
        } else {
            // Print multiline with indentation
            self.write("\n");
            self.indent_level += 1;
            for (i, complex) in list.selectors.iter().enumerate() {
                if i > 0 {
                    self.write(",\n");
                }
                self.write_indent();
                self.print_complex_selector(complex);
            }
            self.write("\n");
            self.indent_level -= 1;
            self.write_indent();
        }
    }

    /// Internal implementation of selector list formatting
    ///
    /// - `nested`: if true, never wrap (for :is(), :where(), :not() arguments)
    /// - if false, wrap top-level selector lists with 2+ selectors (prettier's rule)
    fn print_selector_list_internal(&mut self, list: &internal::SelectorList, nested: bool) {
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
            self.print_selector_list_with_wrapping(list, nested);
        }
    }

    /// Format a selector list with optional line wrapping
    ///
    /// Prettier's rule for top-level selector lists: ALWAYS break with 2+ selectors.
    /// Nested selector lists (in :is(), :where(), etc.) never wrap.
    fn print_selector_list_with_wrapping(&mut self, list: &internal::SelectorList, nested: bool) {
        if list.selectors.is_empty() {
            return;
        }

        // Prettier's rule: top-level selector lists with 2+ selectors ALWAYS break
        // Nested selector lists (in pseudo-classes) NEVER break
        let should_break = !nested && list.selectors.len() >= 2;

        if should_break {
            // Print multiline: each selector on its own line
            for (i, complex) in list.selectors.iter().enumerate() {
                if i > 0 {
                    self.write(",");
                    self.write("\n");
                    self.write_indent();
                }
                self.print_complex_selector(complex);
            }
        } else {
            // Print inline: ", " between selectors
            for (i, complex) in list.selectors.iter().enumerate() {
                if i > 0 {
                    self.write(", ");
                }
                self.print_complex_selector(complex);
            }
        }
    }

    /// Format a complex selector (relative selectors with combinators)
    ///
    /// Supports line wrapping for long selectors (>100 chars):
    /// - If selector fits on one line: print inline
    /// - If too long: break at combinators with indentation
    pub(super) fn print_complex_selector(&mut self, complex: &internal::ComplexSelector) {
        use tsv_lang::doc;

        // Single selector part - always print inline
        if complex.children.len() == 1 {
            self.print_relative_selector_internal(&complex.children[0], true, false);
            return;
        }

        // Build a doc for the entire complex selector to check if it fits
        let selector_doc = self.build_complex_selector_doc(complex);

        // Check if it fits on one line
        // Account for: indent (1 tab = 2 chars) + trailing " {" (2 chars) = 4 chars overhead
        // Tab width is counted based on config.tab_width (default: 2)
        let indent_width = self.indent_level * self.config.tab_width;
        let overhead = indent_width + 2; // " {" or ", "
        let available_width = self.config.print_width.saturating_sub(overhead);
        let fits = doc::fits(
            &selector_doc,
            available_width,
            doc::Mode::Flat,
            &self.config,
        );

        if fits {
            // Print inline
            for (i, relative) in complex.children.iter().enumerate() {
                let is_first = i == 0;
                self.print_relative_selector_internal(relative, is_first, false);
            }
        } else {
            // Print with line breaks at combinators
            for (i, relative) in complex.children.iter().enumerate() {
                let is_first = i == 0;

                if !is_first {
                    // Break before combinator with indentation
                    self.write("\n");
                    self.indent_level += 1;
                    self.write_indent();
                    self.indent_level -= 1;
                }

                // When wrapping, all parts after the first are at line start (no leading space needed)
                self.print_relative_selector_internal(relative, is_first, !is_first);
            }
        }
    }

    /// Build a doc representation of a selector list for width checking
    fn build_selector_list_doc(&self, list: &internal::SelectorList) -> tsv_lang::doc::Doc {
        use tsv_lang::doc;

        let mut parts = Vec::new();
        for (i, complex) in list.selectors.iter().enumerate() {
            if i > 0 {
                parts.push(doc::text(", "));
            }
            parts.push(self.build_complex_selector_doc(complex));
        }
        doc::concat(parts)
    }

    /// Build a doc representation of a complex selector for width checking
    fn build_complex_selector_doc(
        &self,
        complex: &internal::ComplexSelector,
    ) -> tsv_lang::doc::Doc {
        use tsv_lang::doc;

        let mut parts = Vec::new();

        for (i, relative) in complex.children.iter().enumerate() {
            let is_first = i == 0;

            // Add combinator if present
            if let Some(combinator) = &relative.combinator {
                let combinator_text = match combinator {
                    internal::Combinator::Descendant => {
                        if is_first {
                            String::new()
                        } else {
                            " ".to_string()
                        }
                    }
                    internal::Combinator::Child => {
                        if is_first {
                            "> ".to_string()
                        } else {
                            " > ".to_string()
                        }
                    }
                    internal::Combinator::NextSibling => {
                        if is_first {
                            "+ ".to_string()
                        } else {
                            " + ".to_string()
                        }
                    }
                    internal::Combinator::SubsequentSibling => {
                        if is_first {
                            "~ ".to_string()
                        } else {
                            " ~ ".to_string()
                        }
                    }
                    internal::Combinator::Column => {
                        if is_first {
                            "|| ".to_string()
                        } else {
                            " || ".to_string()
                        }
                    }
                };
                if !combinator_text.is_empty() {
                    parts.push(doc::text(combinator_text));
                }
            }

            // Add simple selectors
            for simple in &relative.selectors {
                parts.push(doc::text(self.simple_selector_to_string(simple)));
            }
        }

        doc::concat(parts)
    }

    /// Convert a simple selector to a string for doc building
    fn simple_selector_to_string(&self, simple: &internal::SimpleSelector) -> String {
        match simple {
            internal::SimpleSelector::Type { span, .. } => span.extract(self.source).to_string(),
            internal::SimpleSelector::Universal { namespace, .. } => {
                if let Some(ns) = namespace {
                    format!("{ns}|*")
                } else {
                    "*".to_string()
                }
            }
            internal::SimpleSelector::Class { span, .. } => span.extract(self.source).to_string(),
            internal::SimpleSelector::Id { span, .. } => span.extract(self.source).to_string(),
            internal::SimpleSelector::Attribute {
                namespace,
                name,
                matcher,
                value,
                flags,
                ..
            } => {
                let mut result = String::from("[");
                if let Some(ns) = namespace {
                    result.push_str(ns);
                    result.push('|');
                }
                result.push_str(name);
                if let Some(m) = matcher {
                    result.push_str(m.as_str());
                    if let Some(v) = value {
                        result.push('\'');
                        result.push_str(v);
                        result.push('\'');
                    }
                }
                if let Some(f) = flags {
                    result.push(' ');
                    result.push_str(f);
                }
                result.push(']');
                result
            }
            internal::SimpleSelector::PseudoClass { span, .. } => {
                // For width calculation, extract from source to get accurate length
                // This includes the pseudo-class name and all its arguments
                span.extract(self.source).to_string()
            }
            internal::SimpleSelector::PseudoElement { span, .. } => {
                // For width calculation, extract from source to get accurate length
                span.extract(self.source).to_string()
            }
            internal::SimpleSelector::Nesting { .. } => "&".to_string(),
            internal::SimpleSelector::Percentage { value, .. } => format!("{value}%"),
            internal::SimpleSelector::Invalid { raw, .. } => raw.to_string(),
        }
    }

    /// Internal helper to format a relative selector with context
    ///
    /// - `is_first_in_complex`: true if this is the first relative selector in a complex selector
    /// - `at_line_start`: true if this selector is at the start of a line (after a line break)
    fn print_relative_selector_internal(
        &mut self,
        relative: &internal::RelativeSelector,
        is_first_in_complex: bool,
        at_line_start: bool,
    ) {
        // Print combinator if present
        if let Some(combinator) = &relative.combinator {
            // Leading combinator: first selector in a complex selector with a combinator,
            // or at start of line (after line break in wrapped selector)
            // Example: :has(> img) - the > is leading (no space before)
            // Example (wrapped): <newline><indent>> .class - the > is leading (no space before)
            // Between combinator: subsequent selectors in a complex selector on same line
            // Example: div > span - the > is between (space before and after)
            let is_leading = is_first_in_complex || at_line_start;

            match combinator {
                internal::Combinator::Descendant => {
                    // Descendant combinator is just a space
                    // When at line start (wrapping), skip it - the line break serves as the separator
                    if !at_line_start {
                        self.write(" ");
                    }
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
                    self.write(m.as_str());
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
                self.write(&format!("{value}%"));
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
                    self.print_selector_list_nested(selectors);
                }
            }
            internal::PseudoClassArgs::SelectorList { selectors, .. } => {
                self.print_selector_list_nested(selectors);
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
