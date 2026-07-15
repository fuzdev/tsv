<script lang="ts">
	// case A: first union member with leading line comment
	type A = (// leading
		a) | b;

	// case B: multiple leading line comments
	type B = (// c1
		// c2
		a) | b;

	// case C: leading line + trailing block
	type C = (// leading
		a /* t */) | b;

	// case D: object-literal first member - the comment run takes the offset, and
	// the object's own lines sit at its own indent (closing `}` aligns with `{`)
	type D = (// leading
		{
			a: 1;
			b: 2;
		}) | b;

	// case E: default-paren first member - same as D, the paren layout supplies
	// the member's own indent
	type E = | // leading
		(a & {
			a: 1;
			b: 2;
		}) | b;
</script>
