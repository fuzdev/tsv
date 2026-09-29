<script lang="ts"></script>

{#snippet fn1(a = (x as T).y)}<span>{a}</span>{/snippet}

{#snippet fn2(a = (x satisfies T).y)}<span>{a}</span>{/snippet}

{#snippet fn3(a = (x as T)!)}<span>{a}</span>{/snippet}

{#snippet fn4(a = (b?.c)!.d)}<span>{a}</span>{/snippet}

{#snippet fn5(a = (f<T>).x)}<span>{a}</span>{/snippet}

{#snippet fn6(a: number = (x + y) * 2)}<span>{a}</span>{/snippet}

{#snippet fn7(a = (x satisfies T) ? y : z)}<span>{a}</span>{/snippet}
