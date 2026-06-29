<script lang="ts">
	// Line comment after a tuple rest `...`, a named member `:`, and an optional
	// member `?:`, the element type on the next line.
	type T = [
		...// c
		U
	];
	type N = [
		x: // c
		U
	];
	type O = [
		x?: // c
		U
	];
</script>
