<script lang="ts">
	// Object literal as first intersection member
	type T = {
		a: A;
	} & B;

	// Multiple properties
	type U = {
		a: A;
		b: B;
		c: C;
	} & D;

	// Multiple trailing intersection members
	type V = {
		a: A;
	} & B &
		C;

	// Used in variable type annotation
	let { a, b }: { a: A } & B = $props();
</script>
