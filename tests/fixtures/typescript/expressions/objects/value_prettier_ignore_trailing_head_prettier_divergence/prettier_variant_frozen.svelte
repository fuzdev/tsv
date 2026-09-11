<script lang="ts">
	// a directive trailing an object property's `:` shares its line with the key, so it is
	// inert: the value formats normally
	const o = {
		k: // prettier-ignore
			fn([
			0, 0,
			1, 0
		]),
		m: 1
	};

	// the declarator `=` twin, where both formatters leave the directive inert
	const a = // prettier-ignore
		fn([0, 0, 1, 0]);
</script>
