<!--
	A CSS hex escape takes ONE optional whitespace terminator, and `read_identifier` matches
	it with `(\r\n|\s)?` - a JS regex, so the class is JS `\s` and not Rust's
	`char::is_whitespace`. Two readers must agree on it or the wire disagrees with the span
	it was cut from: the lexer's `decode_unicode_escape`, which decides the token BOUNDARY,
	and the wire writer's `raw_selector_name`. The terminators below are INVISIBLE - a
	U+FEFF, a plain space (the control), and a U+00A0 - and each is eaten by the escape, so
	the three selectors are `aAb`, `cAd` and `eAf`. Retyping any of them as a plain space
	silently reduces this to three copies of the control. The mirror witness, a U+0085 NEL,
	cannot be a fixture at all (canonical REJECTS it, so there is no oracle) and is pinned
	in `tests/css_boundary_whitespace.rs` instead.
-->
<style>
	.a\41﻿b {
		color: red;
	}

	.c\41 d {
		color: blue;
	}

	.e\41 f {
		color: green;
	}
</style>
