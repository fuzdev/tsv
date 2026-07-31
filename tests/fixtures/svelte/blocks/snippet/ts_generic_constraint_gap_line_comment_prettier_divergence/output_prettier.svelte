<script lang="ts">
	type X = string;
</script>

{#snippet fn<
	T extends // c
		X
>(a: T)}
	<span>{a}</span>
{/snippet}
