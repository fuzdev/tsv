<script lang="ts">
	// trailing comment in a retained parenthesized intersection member
	type A1 = (a & b /* c */) | c;

	// leading comment in a retained parenthesized intersection member
	type A2 = a | (/* c */ b & c);

	// trailing comment in a retained parenthesized intersection member (non-first)
	type A3 = a | (b & c /* c */);

	// the same gap where the intersection's last member is an object literal
	type A4 = (a & { x: X } /* c */) | c;
	type A5 = (a & {} /* c */) | c;

	// and where the parenthesized intersection is an optional tuple element
	type A6 = [(a & { x: X } /* c */)?];

	// a line comment in that gap keeps its line, dropping `)` to the next — under the
	// `(`, the column the object's own `})` closer takes when it breaks
	type A7 =
		| (a & { x: X } // c
		  )
		| c;
</script>
