<script lang="ts">
	// a trailing // prettier-ignore freezes the PRECEDING member in prettier; tsv is inert to
	// it (both members reformat normally) — prettier_variant_frozen pins prettier's frozen form.
	// object interiors in BOTH members make any misbound freeze visible either direction
	type U =
		| ({ x: 1 } & a2) // prettier-ignore
		| ({ y: 2 } & b2);
</script>
