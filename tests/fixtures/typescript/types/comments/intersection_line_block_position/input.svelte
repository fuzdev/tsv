<script lang="ts">
	// a line comment forces multiline; block comments keep their authored side of each &
	// before-& block trails the previous member (before the &); object-adjacency still glues
	type T1 = A /* b1 */ &
		B & { c: C } & D & // trail
		E;

	// after-& block leads the next member (on its line at a break)
	type T2 = A &
		/* b2 */ B &
		C & // trail
		D;

	// before-& block trailing an object member, glued to the next
	type T3 = { a: A } /* b3 */ & B & // trail
		C;

	// after-& block leading an object member, inline at a glue boundary
	type T4 = A &
		B & /* b4 */ { c: C } & D & // trail
		E;
</script>
