<script lang="ts">
	// the first member's parenthesized leading line comment hoists out of the intersection;
	// a line comment between later members must be preserved, not dropped
	type T1 = [
		// h1
		a &
			b & // c1
			d
	];

	// hoisting first member is a parenthesized union — parens kept for precedence
	type T2 = [
		// h2
		(A | B) &
			C & // c2
			D
	];
</script>
