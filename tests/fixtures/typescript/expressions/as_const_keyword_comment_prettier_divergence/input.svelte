<script lang="ts">
	// Block comment between `as` and `const` stays put (cf. `as <type>`)
	1 as /* c */ const;

	// Line comment after `as` keeps `const` on the next line
	1 as // c
		const;
</script>
