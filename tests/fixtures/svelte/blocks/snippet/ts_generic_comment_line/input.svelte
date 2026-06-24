<script lang="ts">
	type X = string;
</script>

<!-- line comment inside generics forces the list to wrap; body drops (block-authored) -->
{#snippet fn<
	T // c
>(a: T)}
	<div>x</div>
{/snippet}
