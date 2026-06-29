<script lang="ts">
	// Line comment after a tuple rest `...`, a named member `:`, and an optional
	// member `?:`, the element type on the next line.
	type T = [
		...// c
		U
	];
	type N = [
		x: U // c
	];
	type O = [
		x?: U // c
	];
</script>
