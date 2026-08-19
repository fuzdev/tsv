<script lang="ts">
	// an optional tuple element whose intersection ends in an object literal: the pair the
	// `?` requires supplies the aligned trailing-object layout, and the head shell's `//`
	// lands in that pair's own `(` gap - on the `(` line the author glued it to
	type A = [
		// c1
		(B & {
			a: 1;
		})?
	];

	// the same comment written on its own line inside the shell keeps that line
	type C = [
		// c2
		(D & {
			a: 1;
		})?
	];

	// the shell one link further in, under an array suffix
	type E = [
		// c3
		(F[] & {
			a: 1;
		})?
	];

	// the bare authoring of the same comment, written directly in the pair's own gap -
	// the fixed point every spelling above settles on
	type G = [
		// c4
		(H & {
			a: 1;
		})?
	];
</script>
