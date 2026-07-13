<style>
	/* identity escapes keep the backslash in the AST name */
	#\? {
		color: red;
	}
	.\@ {
		color: blue;
	}
	a\+b {
		color: green;
	}
	/* hex escapes decode (with optional whitespace terminator) */
	#\3A b {
		color: orange;
	}
	/* mixed identity and hex in one name */
	.a\?\41 b {
		color: purple;
	}
	/* astral-plane (>BMP) hex escape decodes to a multi-byte char in the AST name
	   (exercises surrogate-pair / 4-byte offset translation); formatter keeps it raw */
	.\1F600 b {
		color: teal;
	}
	/* a hex escape's terminator whitespace right before the block is part of the
	   selector token, but the formatter must not re-emit it as a doubled separator */
	.\41 {
		color: brown;
	}
	/* the same terminator before a combinator is consumed once, not doubled — and
	   the single space inside the compound `.\41 .b` (two simple selectors) stays */
	.\41 .b > c\64 e {
		color: navy;
	}
	/* pseudo-class and pseudo-element names keep hex escapes raw too — Svelte
	   decodes them in the AST `name`, but the formatter preserves the source */
	:\41 {
		color: pink;
	}
	::\41 b {
		color: gray;
	}
	/* the terminator between two escaped pseudos in a compound stays; only the
	   trailing one (before the block) is dropped */
	:\41 :\42 {
		color: olive;
	}
	/* a LITERAL escape's whitespace is the opposite of a hex terminator: the space is
	   the escape's PAYLOAD (the class is named `a `), so it must survive. Dropping it
	   strands the backslash onto the next separator, merging the descendant combinator
	   into the compound (`.a\  .b` would re-parse as one compound, losing the space) */
	.a\  .b {
		color: maroon;
	}
	/* the same escape with NO combinator after it: one compound (class `a ` and .b) */
	.a\ .b {
		color: lime;
	}
	/* and before a selector-list comma / the block, where a stranded backslash would
	   escape the `,` / `{` instead. Before `{` the payload space IS the separator, so
	   no second one is added (the literal-escape twin of the hex-terminator rule) */
	.a\ ,
	.c {
		color: aqua;
	}
	.a\ {
		color: silver;
	}
</style>
