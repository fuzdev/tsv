<script lang="ts">
	// case A: first union member with leading line comment
	type A = (// leading
		a) | b;

	// case B: later union member with leading line comment
	type B = a | (// leading
		b);

	// case D: multiple leading line comments
	type D = (// c1
		// c2
		a) | b;

	// case E: leading line + trailing block
	type E = (// leading
		a /* t */) | b;

	// case F: paren retained (intersection in union)
	type F = a | (// leading
		b & c);

	// case G: conditional extends
	type G = a extends (// comment
		b) ? c : d;

	// case M1: 3-member union, leading on middle
	type M1 = a | (// leading
		b) | c;

	// case M2: 3-member union, leading on last
	type M2 = a | b | (// leading
		c);
</script>
