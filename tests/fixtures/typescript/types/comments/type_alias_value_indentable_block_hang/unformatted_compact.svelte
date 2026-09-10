<script lang="ts">
	// An INDENTABLE multi-line block comment leading the value hangs it under the `=`,
	// whatever layout the value or the type parameters would otherwise take — the type
	// alias is a `printAssignment` site like every other one.

	// a bare reference
	type A = /** c
	 */ B;

	// a literal
	type C = /** c
	 */ 'd';

	// an object type
	type E = /** c
	 */ { f: A };

	// a tuple
	type G = /** c
	 */ [A, B];

	// a reference with type arguments
	type H = /** c
	 */ I<A>;

	// a function type
	type J = /** c
	 */ (k: A) => B;

	// an intersection
	type L = /** c
	 */ A & B;

	// a conditional
	type M = /** c
	 */ A extends B ? C : D;

	// a hugged union, whose object member still owns its own expansion
	type N = /** c
	 */ { o: A } | null;

	// complex type parameters, which break on their own account and not here
	type P<Q extends string, R = number> = /** c
	 */ A;

	// the head of a glued run
	type S = /** c
	 */ /* c1 */ A;

	// ...and its tail
	type T = /* c1 */ /** c
	 */ A;

	// Contrast: a PRESERVED (non-indentable) block keeps the value on the `=` line.
	type U = /* c1
	c2 */ A;

	// Contrast: so does a single-line block.
	type V = /* c */ A;

	// Contrast: a comment the author gave its own line keeps it, and the value drops
	// below rather than gluing to the comment's closing line.
	type W = /** c
	 */
		A;

	// Contrast: a line comment takes that same own-line shape.
	type X =
	// c
		A;
</script>
