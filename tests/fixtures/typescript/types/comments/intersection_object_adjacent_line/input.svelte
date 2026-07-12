<script lang="ts">
	// trailing line comment; object-adjacent members stay glued, breaks at neither-object boundaries
	type T1 = A &
		B & { c: C } & D & // c1
		E;

	// own-line line comment before the last member; earlier members stay glued
	type T2 = A &
		B & { c: C } & D &
		// c2
		E;

	// own-line line comment leading the object forces only that boundary to break
	type T3 = A &
		B &
		// c3
		{ c: C } & D &
		E;

	// own-line block comment between members; object-adjacent members still glue
	type T4 = A &
		B & { c: C } & D &
		/* c4 */
		E;

	// object first; the member after the neither-object boundary indents
	type T5 = { c: C } & B & // c5
		D;

	// line comment at the first neither-object boundary; the rest stays glued
	type T7 = A & // c7
		B & { c: C } & D;
</script>
