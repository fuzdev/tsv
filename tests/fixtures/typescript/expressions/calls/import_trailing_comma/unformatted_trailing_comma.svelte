<script lang="ts">
	// dynamic import, source only — a trailing comma is valid per the ES spec and stripped on format
	import('./a',);

	// dynamic import with an options arg — a trailing comma after it is valid too and stripped
	import('./b', { with: { type: 'json' } },);
</script>
