<script lang="ts">
	// Line comment trailing a tuple element
	type T = [
		A, // c
		B
	];

	// Labeled tuple element
	type U = [
		a: A, // c
		b: B
	];

	// A same-line comment trails the comma; an own-line comment leads the next
	// element. The comma binds to the element either way (no swallow).
	type V = [
		A, // c1
		// c2
		B
	];
</script>
