<script lang="ts">
	// Conditional check: comment after `extends`, check type on the next line
	type C = T extends U // c
		? X
		: Y;
</script>
