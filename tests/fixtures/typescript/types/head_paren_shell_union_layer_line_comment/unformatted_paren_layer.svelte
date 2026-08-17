<script lang="ts">
	// a conditional's `extends` gap, over a union whose FIRST member is the shell
	type A = B extends ((
	// c1
	C) | D) ? E : F;

	// control: the same union at a `?` branch gap
	type G = H extends I ? ((
	// c2
	J) | K) : L;

	// control: the same union as a type-alias RHS
	type M = ((
	// c3
	N) | O);
</script>
