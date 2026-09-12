<script lang="ts">
	// a frozen arrow body whose slice OPENS with `{` takes a paren pair around the whole
	// body — the unfrozen form puts one inside it instead, and a verbatim slice has no
	// inside to reach
	const aaa = () =>
		// prettier-ignore
		({bbb:   1} | ccc);
</script>
