<script lang="ts">
	// Line comment after `as` stays after the keyword, type on the next line.
	const a = x as // c
		A;

	// Same for `satisfies`.
	const b = x satisfies // c
		B;
</script>
