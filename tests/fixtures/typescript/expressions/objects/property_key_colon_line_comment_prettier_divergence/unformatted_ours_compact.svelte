<script lang="ts">
	const o = {
		a // c
		: 1
	};

	// With a leading comment present too, prettier hoists the before-`:` comment to
	// stack above the key on its own line — distinct, no merge (unlike the trailing
	// `=`/`?` relocations, which collide). tsv keeps it after the key.
	const p = {
		// leading
		b // c1
		: 2
	};
</script>
