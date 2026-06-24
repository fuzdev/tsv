<script lang="ts">
	type Aaaaaaaa = string;
	type Bbbbbbbb = number;
	type Aaaaaaaaaaaaaaaaaaaaaa = string;
</script>

<!-- generics fit (head exactly 100 chars) - stay inline, block body drops -->
{#snippet fn1<Taaaaaaaaa extends Aaaaaaaa, Ubbbbbbbbb extends Bbbbbbbb, Vccccccccccccccccccccccc>()}
	<div>x</div>
{/snippet}

<!-- generics overflow (head 101 chars) - each generic wraps to its own line -->
{#snippet fn2<Taaaaaaaaaa extends Aaaaaaaa, Ubbbbbbbbb extends Bbbbbbbb, Vccccccccccccccccccccccc>()}
	<div>x</div>
{/snippet}

<!-- generics wrap, params stay inline on the closing `>(...)` line -->
{#snippet fn3<Taaaaaaaaaa extends Aaaaaaaaaaaaaaaaaaaaaa, Ubbbbbbbbbb extends Aaaaaaaaaaaaaaaaaaaaaa>(a: Taaaaaaaaaa)}
	<div>x</div>
{/snippet}
