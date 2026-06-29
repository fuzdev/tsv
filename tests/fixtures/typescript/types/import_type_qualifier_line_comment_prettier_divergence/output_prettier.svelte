<script lang="ts">
	// Line comment in an import type's `.`→qualifier gap, the qualifier on the next line.
	type T = import(
		'x' // c
	).Y;
</script>
