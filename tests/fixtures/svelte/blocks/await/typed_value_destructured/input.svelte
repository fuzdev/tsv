<script lang="ts">
	let promise: Promise<{ a: number }> = Promise.resolve({ a: 1 });
</script>

{#await promise}
	<p>text</p>
{:then { a }: { a: number }}
	<p>{a}</p>
{:catch { message }: Error}
	<p>{message}</p>
{/await}
