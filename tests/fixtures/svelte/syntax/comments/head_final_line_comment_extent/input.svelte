<!--
	A line comment runs to the line terminator, so the TRAILING SPACE in each `// c ` below
	belongs to the comment's own text: its value is ` c ` and its end is one past the space.
	That space is the claim of this fixture and it is invisible — trimming these lines "for
	tidiness" turns every case into the glued null control at the bottom rather than failing
	here. The `prettier-ignore` is what lets them survive: both formatters trim a line
	comment's trailing whitespace, so an unfrozen spelling of this file is not a fixed point.
	Every reader that hands the sub-parser a region is here, whether or not it needs to be,
	because the extent is one rule. The `lang="ts"` is for the `{#each xs as A[] … as x}`
	case alone, whose second `as` is a type assertion and whose iterable is re-parsed.
-->
<script lang="ts">
	let xs = [1];
</script>

<!-- prettier-ignore -->
<div>
	{@html expr // c 
	}
	{@render fn() // c 
	}
	{@debug expr // c 
	}
	{#if expr}{@const a = 1 // c 
	}{/if}
	<span {@attach fn // c 
	}></span>
	{#each xs as x (x // c 
	)}{/each}
	{#key expr // c 
	}{/key}
	{#if expr // c 
	}{/if}
	{#await p // c 
	}{/await}
	{expr // c 
	}
	<span attr={expr // c 
	}></span>
	<span {...expr // c 
	}></span>
	{#each xs as A[] // c 
	as x}{/each}
	{@html expr // c
	}
</div>
