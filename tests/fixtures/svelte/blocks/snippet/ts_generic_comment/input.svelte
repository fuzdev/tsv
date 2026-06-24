<script lang="ts">
	type T = number;
</script>

<!-- block comment inside the generic parameter list -->
{#snippet fn<T /* c */>(a: T)}<span>{a}</span>{/snippet}
