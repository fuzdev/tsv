<script lang="ts">
	let y:unknown;
</script>

<!-- an `as` inside a function body is an assertion, not the each separator -->
{#each function(){return y as A;} as item}
	<div>{item}</div>
{/each}

<!-- inside an arrow's block body -->
{#each ()=>{return y as A;} as item}
	<div>{item}</div>
{/each}

<!-- in a class field's computed key and its initializer -->
{#each class{[y as A]=y as A;} as item}
	<div>{item}</div>
{/each}

<!-- inside a class static block -->
{#each class{static{y as A;}} as item}
	<div>{item}</div>
{/each}
