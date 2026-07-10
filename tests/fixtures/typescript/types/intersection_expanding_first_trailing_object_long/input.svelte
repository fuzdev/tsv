<script lang="ts">
	// Object-first intersection with a later object: a trailing object at index >= 2 is
	// preceded by a non-object, so the `was_indented` latch is set and it carries the
	// continuation indent when it breaks - hugging `& {` rather than dropping to a new line.
	// 100 chars effective - trailing object stays inline (at boundary)
	type A100 = { a: A } & BBBBBBBBBB & { cccccccccc: C; dddddddddd: D; eeeeeeeeee: E; fffffffff: F };

	// 101 chars effective - trailing object breaks, indented one level under the continuation
	type A101 = { a: A } & BBBBBBBBBB & {
			cccccccccc: C;
			dddddddddd: D;
			eeeeeeeeee: E;
			ffffffffff: F;
		};
</script>
