<script lang="ts">
	// Chain ending with member access - object hugs inline
	aaaa.bbbb.cccc({
		prop:'value',
	}).length;

	// Longer chain - still hugs
	aaaaaaaaa.bbbbbbbbb.ccccccccc({
		prop:'value',
	}).length;

	// Very long chain - breaks eventually
	aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa.bbbbbbbbbbbbbbbbbbb.ccccccccccccccccccc({
		prop:'value',
	}).length;
</script>
