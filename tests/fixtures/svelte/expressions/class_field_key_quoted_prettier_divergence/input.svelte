<script lang="ts">
	let x = 1;
</script>

<!-- A class expression in a template expression keeps a quoted field key quoted -->
<p>
	{new (class {
		'a': string;
		'b' = 1;
		static 'c' = 2;
		m() {}
	})().b}
</p>
{#if x}
	{@const C = class {
		'a' = 1;
	}}
	<p data-attr={new C().a}>text</p>
{/if}
