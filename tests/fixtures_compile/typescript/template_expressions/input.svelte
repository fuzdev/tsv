<script lang="ts">
	let { a, f }: { a: string; f: <T>(v: T) => T } = $props();
</script>

<p>{a as string}</p>
<p>{a!}</p>
<p>{a satisfies string}</p>
<p>{f<string>(a)}</p>
<p>{@html a as string}</p>
