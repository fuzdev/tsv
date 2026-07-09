<script lang="ts">
	// Import type as a single type argument - `<` opens type args, not a comparison
	const fromImport = f<import('m').T>(firstArg);

	// Construct-signature type argument (`new (...) => R`)
	const fromConstruct = f<new () => T>(firstArg);

	// Abstract construct-signature type argument
	const fromAbstract = f<abstract new () => T>(firstArg);
</script>
