// CSS selector formatting
//
// Handles formatting of:
// - Selector lists (comma-separated)
// - Complex selectors (with combinators)
// - Relative selectors (simple selector chains)
// - Simple selectors (type, class, id, pseudo-class, pseudo-element, etc.)
//
// ## Architecture
//
// This module uses a doc-first approach where all formatting logic lives in
// `build_*_doc()` methods. The `print_*` methods use these doc builders and
// handle wrapping decisions.

use super::Printer;
use crate::ast::internal;
use tsv_lang::doc::{self, Mode, arena::DocId};

impl<'a> Printer<'a> {
    /// Get the string representation of a combinator
    ///
    /// - `is_leading`: true for first selector in complex, or at line start after wrap
    ///
    /// Returns the combinator string with appropriate spacing.
    /// For Descendant, returns "" when leading (caller handles the space/linebreak).
    fn get_combinator_str(combinator: internal::Combinator, is_leading: bool) -> &'static str {
        match (combinator, is_leading) {
            (internal::Combinator::Descendant, true) => "",
            (internal::Combinator::Descendant, false) => " ",
            (internal::Combinator::Child, true) => "> ",
            (internal::Combinator::Child, false) => " > ",
            (internal::Combinator::NextSibling, true) => "+ ",
            (internal::Combinator::NextSibling, false) => " + ",
            (internal::Combinator::SubsequentSibling, true) => "~ ",
            (internal::Combinator::SubsequentSibling, false) => " ~ ",
            (internal::Combinator::Column, true) => "|| ",
            (internal::Combinator::Column, false) => " || ",
        }
    }

    /// Print a selector list inline with `, ` separators
    fn print_selector_list_inline(&mut self, list: &internal::SelectorList) {
        for (i, complex) in list.selectors.iter().enumerate() {
            if i > 0 {
                self.write(", ");
            }
            self.print_complex_selector(complex);
        }
    }

    /// Normalize spacing around comments in selector source text
    ///
    /// Ensures proper spacing:
    /// - Space after comma before comment: `,/*` → `, /*`
    /// - Space after comment before class selector: `*/.` → `*/ .`
    /// - Reduce double spaces: `,  /*` → `, /*`, `*/  .` → `*/ .`
    fn normalize_selector_comment_spacing(source_text: &str) -> String {
        source_text
            .replace(",/*", ", /*")
            .replace("*/.", "*/ .")
            .replace(",  /*", ", /*")
            .replace("*/  .", "*/ .")
    }

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
        if list.selectors.is_empty() {
            return;
        }

        // Check if source contains comments
        let source_text = list.span.extract(self.source);
        if source_text.contains("/*") {
            let normalized = Self::normalize_selector_comment_spacing(source_text);
            self.write(&normalized);
            return;
        }

        // Build a doc for the selector list to check if it fits
        let list_doc = self.build_selector_list_doc(list);

        // Check if it fits on one line
        // Use current column position (what's already printed: indent + pseudo-class prefix)
        // and leave room for closing `) {` (3 chars)
        let current_col = self.current_column();
        let available_width = tsv_lang::PRINT_WIDTH.saturating_sub(current_col + 3);
        let fits = doc::arena_fits::<dyn doc::TextResolver>(
            &self.arena,
            list_doc,
            available_width,
            Mode::Flat,
            None,
        );

        if fits {
            // Print inline
            self.print_selector_list_inline(list);
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
        if source_text.contains("/*") {
            let normalized = Self::normalize_selector_comment_spacing(source_text);
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
            self.print_selector_list_inline(list);
        }
    }

    /// Format a complex selector (relative selectors with combinators)
    ///
    /// Supports line wrapping for long selectors (>100 chars):
    /// - If selector fits on one line: print inline
    /// - If too long: break at combinators with indentation
    pub(super) fn print_complex_selector(&mut self, complex: &internal::ComplexSelector) {
        // Single selector part - always print inline
        if complex.children.len() == 1 {
            self.print_relative_selector_internal(&complex.children[0], true, false);
            return;
        }

        // Build a doc for the entire complex selector to check if it fits
        let selector_doc = self.build_complex_selector_doc(complex);

        // Check if it fits on one line
        // Account for: indent + trailing " {" (2 chars)
        let overhead = self.indent_width() + 2; // " {" or ", "
        let available_width = tsv_lang::PRINT_WIDTH.saturating_sub(overhead);
        let fits = doc::arena_fits::<dyn doc::TextResolver>(
            &self.arena,
            selector_doc,
            available_width,
            Mode::Flat,
            None,
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
    pub(crate) fn build_selector_list_doc(&self, list: &internal::SelectorList) -> DocId {
        let d = self.d();
        let sep = d.text(", ");
        d.join_doc(
            list.selectors
                .iter()
                .map(|complex| self.build_complex_selector_doc(complex)),
            sep,
        )
    }

    /// Build a doc representation of a complex selector for width checking
    fn build_complex_selector_doc(&self, complex: &internal::ComplexSelector) -> DocId {
        let docs: Vec<_> = complex
            .children
            .iter()
            .enumerate()
            .map(|(i, relative)| self.build_relative_selector_doc(relative, i == 0))
            .collect();
        self.d().concat(&docs)
    }

    //
    // Doc Builders - all formatting logic expressed as doc IR
    //

    /// Build a doc for a simple selector
    ///
    /// Uses source extraction where possible to preserve escapes.
    pub(crate) fn build_simple_selector_doc(&self, simple: &internal::SimpleSelector) -> DocId {
        let d = self.d();
        match simple {
            internal::SimpleSelector::Type { span, .. } => {
                d.text_owned(span.extract(self.source).to_string())
            }
            internal::SimpleSelector::Universal { namespace, .. } => {
                if let Some(ns) = namespace {
                    d.text_owned(format!("{ns}|*"))
                } else {
                    d.text("*")
                }
            }
            internal::SimpleSelector::Class { span, .. } => {
                d.text_owned(span.extract(self.source).to_string())
            }
            internal::SimpleSelector::Id { span, .. } => {
                d.text_owned(span.extract(self.source).to_string())
            }
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
                d.text_owned(result)
            }
            internal::SimpleSelector::PseudoClass { span, .. } => {
                // Extract from source to get accurate representation
                d.text_owned(span.extract(self.source).to_string())
            }
            internal::SimpleSelector::PseudoElement { span, .. } => {
                d.text_owned(span.extract(self.source).to_string())
            }
            internal::SimpleSelector::Nesting { .. } => d.text("&"),
            internal::SimpleSelector::Percentage { value, .. } => d.text_owned(format!("{value}%")),
            internal::SimpleSelector::Invalid { raw, .. } => d.text_owned(raw.clone()),
        }
    }

    /// Build a doc for a relative selector
    ///
    /// A relative selector is a combinator followed by simple selectors.
    fn build_relative_selector_doc(
        &self,
        relative: &internal::RelativeSelector,
        is_first: bool,
    ) -> DocId {
        let d = self.d();
        let mut parts = Vec::new();

        // Add combinator if present
        if let Some(combinator) = relative.combinator {
            let combinator_text = Self::get_combinator_str(combinator, is_first);
            if !combinator_text.is_empty() {
                parts.push(d.text(combinator_text));
            }
        }

        // Add simple selectors
        for simple in &relative.selectors {
            parts.push(self.build_simple_selector_doc(simple));
        }

        d.concat(&parts)
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
        if let Some(combinator) = relative.combinator {
            // Leading combinator: first selector in a complex selector with a combinator,
            // or at start of line (after line break in wrapped selector)
            // Example: :has(> img) - the > is leading (no space before)
            // Example (wrapped): <newline><indent>> .class - the > is leading (no space before)
            // Between combinator: subsequent selectors in a complex selector on same line
            // Example: div > span - the > is between (space before and after)
            let is_leading = is_first_in_complex || at_line_start;

            // For Descendant at line start, skip the space - the line break serves as separator
            if matches!(combinator, internal::Combinator::Descendant) && at_line_start {
                // Do nothing - line break is the separator
            } else {
                let combinator_text = Self::get_combinator_str(combinator, is_leading);
                if !combinator_text.is_empty() {
                    self.write(combinator_text);
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
                    self.print_pseudo_class_with_args(args, false);
                }
            }
            internal::SimpleSelector::PseudoElement { name, args, .. } => {
                self.write("::");
                self.write(name);
                if let Some(args) = args {
                    self.write("(");
                    self.print_pseudo_element_args(args);
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

    /// Format a pseudo-class with arguments, handling line breaks
    ///
    /// Matches Prettier's behavior:
    /// - Short content: `:is(.a, .b)` (inline)
    /// - Long content: `:is(\n  .a,\n  .b\n)` (broken with indent)
    ///
    /// - `extra_indent`: if true, add extra indentation for nested content (selector-selector >2 nodes)
    fn print_pseudo_class_with_args(
        &mut self,
        args: &internal::PseudoClassArgs,
        extra_indent: bool,
    ) {
        // Build a doc for the args to check if they fit
        let args_doc = self.build_pseudo_class_args_doc(args);

        // Calculate available width: account for `(` and `)` plus trailing content
        // We need to leave room for `) {` (3 chars) at end of selector
        let current_col = self.current_column();
        let available_width = tsv_lang::PRINT_WIDTH.saturating_sub(current_col + 4);
        let fits = doc::arena_fits::<dyn doc::TextResolver>(
            &self.arena,
            args_doc,
            available_width,
            Mode::Flat,
            None,
        );

        // Also check if any nested content would need to break
        // If a selector list has complex selectors with >2 simple selectors,
        // or contains pseudo-classes that would break, we should break the outer too
        let has_complex_content = self.args_have_complex_content(args);

        if fits && !has_complex_content {
            // Print inline
            self.write("(");
            self.print_pseudo_class_args_with_mode(args, false);
            self.write(")");
        } else {
            // Print with line breaks - matches Prettier's group pattern
            // Apply extra_indent to content if this selector-selector has >2 nodes
            self.write("(");
            self.write("\n");
            self.indent_level += 1;
            if extra_indent {
                self.indent_level += 1;
            }
            self.print_pseudo_class_args_with_mode(args, true);
            self.write("\n");
            // Only remove the inner indent, keep extra_indent for closing
            self.indent_level -= 1;
            self.write_indent();
            self.write(")");
            if extra_indent {
                self.indent_level -= 1;
            }
        }
    }

    /// Check if pseudo-class args contain complex content that would cause breaks
    fn args_have_complex_content(&self, args: &internal::PseudoClassArgs) -> bool {
        match args {
            internal::PseudoClassArgs::SelectorList { selectors, .. } => {
                self.selector_list_has_complex_content(selectors)
            }
            internal::PseudoClassArgs::Nth {
                of_selector: Some(selectors),
                ..
            } => self.selector_list_has_complex_content(selectors),
            internal::PseudoClassArgs::Nth { .. } => false,
            _ => false,
        }
    }

    /// Check if a selector list contains complex selectors that would break
    ///
    /// We check for pseudo-classes that would break, not just >2 simple selectors.
    /// The >2 check affects *indentation* when breaking, not *whether* to break.
    fn selector_list_has_complex_content(&self, list: &internal::SelectorList) -> bool {
        for complex in &list.selectors {
            // Check if any simple selector is a pseudo-class with long args
            for rel in &complex.children {
                for simple in &rel.selectors {
                    if let internal::SimpleSelector::PseudoClass {
                        args: Some(args), ..
                    } = simple
                    {
                        // Check if this pseudo-class's args would break
                        let args_doc = self.build_pseudo_class_args_doc(args);
                        // Use a conservative width check
                        let fits = doc::arena_fits::<dyn doc::TextResolver>(
                            &self.arena,
                            args_doc,
                            60,
                            Mode::Flat,
                            None,
                        );
                        if !fits {
                            return true;
                        }
                        // Recursively check nested content
                        if self.args_have_complex_content(args) {
                            return true;
                        }
                    }
                }
            }
        }
        false
    }

    /// Build a doc for pseudo-class args to check if they fit
    fn build_pseudo_class_args_doc(&self, args: &internal::PseudoClassArgs) -> DocId {
        let d = self.d();
        match args {
            internal::PseudoClassArgs::SelectorList { selectors, .. } => {
                self.build_selector_list_doc(selectors)
            }
            internal::PseudoClassArgs::Nth {
                value, of_selector, ..
            } => {
                let normalized = Self::normalize_an_plus_b(value);
                if let Some(selectors) = of_selector {
                    let norm_doc = d.text_owned(normalized);
                    let of_doc = d.text(" of ");
                    let sel_doc = self.build_selector_list_doc(selectors);
                    d.concat(&[norm_doc, of_doc, sel_doc])
                } else {
                    d.text_owned(normalized)
                }
            }
            internal::PseudoClassArgs::Slotted { selectors, .. } => {
                let sel_docs: Vec<_> = selectors
                    .iter()
                    .map(|s| self.build_simple_selector_doc(s))
                    .collect();
                d.concat(&sel_docs)
            }
            internal::PseudoClassArgs::Part { idents, .. } => d.text_owned(idents.join(" ")),
            internal::PseudoClassArgs::Identifier { value, .. } => d.text_owned(value.clone()),
        }
    }

    /// Print pseudo-class args with specified mode
    ///
    /// - `multiline=false`: print on single line with `, ` separators
    /// - `multiline=true`: print with indentation and line breaks
    fn print_pseudo_class_args_with_mode(
        &mut self,
        args: &internal::PseudoClassArgs,
        multiline: bool,
    ) {
        match args {
            internal::PseudoClassArgs::Nth {
                value, of_selector, ..
            } => {
                if multiline {
                    self.write_indent();
                }
                let normalized = Self::normalize_an_plus_b(value);
                self.write(&normalized);
                if let Some(selectors) = of_selector {
                    self.write(" of ");
                    if multiline {
                        self.print_selector_list_nested(selectors);
                    } else {
                        self.print_selector_list_inline(selectors);
                    }
                }
            }
            internal::PseudoClassArgs::SelectorList { selectors, .. } => {
                if multiline {
                    self.print_selector_list_multiline_with_extra_indent(selectors);
                } else {
                    self.print_selector_list_inline(selectors);
                }
            }
            internal::PseudoClassArgs::Slotted { selectors, .. } => {
                if multiline {
                    self.write_indent();
                }
                for selector in selectors {
                    self.print_simple_selector(selector);
                }
            }
            internal::PseudoClassArgs::Part { idents, .. } => {
                if multiline {
                    self.write_indent();
                }
                for (i, ident) in idents.iter().enumerate() {
                    if i > 0 {
                        self.write(" ");
                    }
                    self.write(ident);
                }
            }
            internal::PseudoClassArgs::Identifier { value, .. } => {
                if multiline {
                    self.write_indent();
                }
                self.write(value);
            }
        }
    }

    /// Print selector list in multiline mode with extra indent for complex selectors
    ///
    /// Matches Prettier's behavior: selector-selector with >2 nodes gets extra indent
    /// The extra indent applies to the CONTENT of pseudo-classes within the selector,
    /// not to the selector itself.
    fn print_selector_list_multiline_with_extra_indent(&mut self, list: &internal::SelectorList) {
        for (i, complex) in list.selectors.iter().enumerate() {
            if i > 0 {
                self.write(",\n");
            }

            // Check if this complex selector needs extra indent
            // Prettier: selector-selector with >2 nodes gets indent()
            // Our equivalent: RelativeSelector with >2 simple selectors
            let needs_extra_indent = complex.children.iter().any(|rel| rel.selectors.len() > 2);

            self.write_indent();
            self.print_complex_selector_with_extra_indent(complex, needs_extra_indent);
        }
    }

    /// Print a complex selector, propagating extra indent flag to nested content
    fn print_complex_selector_with_extra_indent(
        &mut self,
        complex: &internal::ComplexSelector,
        extra_indent: bool,
    ) {
        for (i, relative) in complex.children.iter().enumerate() {
            let is_first = i == 0;
            self.print_relative_selector_with_extra_indent(relative, is_first, extra_indent);
        }
    }

    /// Print a relative selector, propagating extra indent flag
    fn print_relative_selector_with_extra_indent(
        &mut self,
        relative: &internal::RelativeSelector,
        is_first: bool,
        extra_indent: bool,
    ) {
        // Print combinator if present
        if let Some(combinator) = relative.combinator {
            let combinator_text = Self::get_combinator_str(combinator, is_first);
            if !combinator_text.is_empty() {
                self.write(combinator_text);
            }
        }

        for simple in &relative.selectors {
            self.print_simple_selector_with_extra_indent(simple, extra_indent);
        }
    }

    /// Print a simple selector, using extra indent for pseudo-class content
    fn print_simple_selector_with_extra_indent(
        &mut self,
        simple: &internal::SimpleSelector,
        extra_indent: bool,
    ) {
        match simple {
            internal::SimpleSelector::PseudoClass { name, args, .. } => {
                self.write(":");
                self.write(name);
                if let Some(args) = args {
                    self.print_pseudo_class_with_args(args, extra_indent);
                }
            }
            // For non-pseudo-class selectors, delegate to the regular printer
            _ => self.print_simple_selector(simple),
        }
    }

    /// Format pseudo-element arguments with auto-wrapping for long selector lists
    ///
    /// Used for pseudo-elements like `::slotted()`, `::part()`, `::highlight()`.
    /// Uses `print_selector_list_nested` which auto-wraps if content is too long.
    fn print_pseudo_element_args(&mut self, args: &internal::PseudoClassArgs) {
        match args {
            internal::PseudoClassArgs::Nth {
                value, of_selector, ..
            } => {
                let normalized = Self::normalize_an_plus_b(value);
                self.write(&normalized);
                if let Some(selectors) = of_selector {
                    self.write(" of ");
                    self.print_selector_list_nested(selectors);
                }
            }
            internal::PseudoClassArgs::SelectorList { selectors, .. } => {
                self.print_selector_list_nested(selectors);
            }
            internal::PseudoClassArgs::Slotted { selectors, .. } => {
                for selector in selectors {
                    self.print_simple_selector(selector);
                }
            }
            internal::PseudoClassArgs::Part { idents, .. } => {
                for (i, ident) in idents.iter().enumerate() {
                    if i > 0 {
                        self.write(" ");
                    }
                    self.write(ident);
                }
            }
            internal::PseudoClassArgs::Identifier { value, .. } => {
                self.write(value);
            }
        }
    }
}
