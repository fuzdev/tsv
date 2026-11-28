<script lang="ts">
	// the directive before a parenthesized nested-union member freezes the whole member,
	// its paren layout as written
	export type a =
		// foo
		| (foo1 & foo2)
		// bar
		| (bar1 & bar2)
		// prettier-ignore
		| (| aaaaaaaaaaaaa&1
    // b
    | bbbbbbbbbbbbb&2)
		// baz
		| (baz1 & baz2);

	// a directive inside the parens applies to the inner list: only the inner member after
	// it freezes, and the later inner members reformat
	export type b =
		// foo
		| (foo1 & foo2)
		// bar
		| (bar1 & bar2)
		// prettier-ignore
		| (| (aaaaaaaaaaaaa&1)
				// b
				| (bbbbbbbbbbbbb & 2))
		| (baz1 & baz2);
</script>
