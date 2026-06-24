<script lang="ts">
	type X = string;
	type Y = number;
</script>

<!-- constrained generic -->
{#snippet fn1<T extends X>(a: T)}<span>{a}</span>{/snippet}

<!-- defaulted generic -->
{#snippet fn2<T = X>(a: T)}<span>{a}</span>{/snippet}

<!-- constraint and default -->
{#snippet fn3<T extends X = Y>(a: T)}<span>{a}</span>{/snippet}

<!-- const modifier -->
{#snippet fn4<const T>(a: T)}<span>{a}</span>{/snippet}

<!-- multiple generics -->
{#snippet fn5<T, U>(a: T, b: U)}<span>{a}{b}</span>{/snippet}
