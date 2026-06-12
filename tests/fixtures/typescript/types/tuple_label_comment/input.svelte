<script lang="ts">
	// Comments between tuple label and ?/: are preserved
	type T = [a /* c1 */?: string];
	type U = [b /* c2 */: string];
</script>
