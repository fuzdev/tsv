<script lang="ts">
	// tsv honors // format-ignore identically to // prettier-ignore; prettier does not
	type A =
		// format-ignore
		(a1 & a2) | (b1 & b2);

	// control: // prettier-ignore is honored by both tools, so it is unchanged in output_prettier
	type B =
		// prettier-ignore
		(c1&c2) | (d1 & d2);
</script>
