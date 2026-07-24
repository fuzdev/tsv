<script lang="ts">
	// a trailing // prettier-ignore freezes the PRECEDING member in prettier; tsv is inert to
	// it (both members reformat normally) — prettier_variant_frozen pins prettier's frozen form
	type U =
		| a1&a2 // prettier-ignore
		| b1&b2;
</script>
