<script lang="ts">
interface A{a:string}
interface B{b:string}
let items:unknown
</script>

<!-- no `as` at all: the plain binding-less head -->
{#each items satisfies A}
<div>text</div>
{/each}

<!-- the assertion run ends on satisfies, so the whole run is the iterable and there is
	still no binding -->
{#each items as A satisfies B}
<div>text</div>
{/each}

<!-- ...with an index -->
{#each items as A[] satisfies B[],i}
<div>{i}</div>
{/each}

<!-- ...with an index and a key -->
{#each items as A satisfies B,i(i)}
<div>{i}</div>
{/each}

<!-- `item` here is a TYPE, not the binding: the satisfies after it ends the run, which
	cancels the as before it -->
{#each items as A[] as item satisfies B}
<div>text</div>
{/each}
