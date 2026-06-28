<script lang="ts">
	// Line comment after = with simple type
	type A = // comment
		B;

	// Block comment after = with simple type
	type B = /* comment */ C;

	// Line comment after = with union
	type D = // comment
		E | F;

	// Block comment after = with union
	type G = /* comment */ H | I;

	// Multiline block comment after = with union
	type J =
		/* comment
		 */
		K | L;

	// Line comment after = with intersection
	type M = // comment
		N & O;

	// Two line comments after = stay on separate lines (not merged)
	type P = // c1
		// c2
		Q | R;
</script>
