<script lang="ts">
	type T = number;
	type U = string;
</script>

<!-- comment before the type annotation -->
{#snippet fn1(a:/* c */ T)}<span>x</span>{/snippet}

<!-- comment before the colon -->
{#snippet fn2(a /* c */:T)}<span>x</span>{/snippet}

<!-- comment trailing a typed param, before the comma -->
{#snippet fn3(a:T /* c */,b:U)}<span>x</span>{/snippet}

<!-- comment after the `=` of a typed default -->
{#snippet fn4(a:T=/* c */ 1)}<span>x</span>{/snippet}

<!-- comment trailing a typed destructuring param -->
{#snippet fn5({a}:T /* c */)}<span>x</span>{/snippet}
