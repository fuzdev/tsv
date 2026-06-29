<script lang="ts">
	// Line comment after a template-literal type's `${`, the interpolation type on
	// the next line.
	type T =
		`a${// c
		B}`;
</script>
