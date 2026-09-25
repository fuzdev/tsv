<!-- template literal lines keep their column, trailing spaces, whitespace-only and empty lines -->
<div>
	<script>
		const a = `x
y`;
		const b = `x   
   

	y`;
		const c = `x${a}
	y`;
	</script>
</div>

<!-- a non-indentable block comment and a line-continued string keep their continuation lines -->
<div>
	<section>
		<script>
			/* a   
b */
			fn();
			const d = /* a
  b */ 1;
			const e =
				'x\
y';
		</script>
	</section>
</div>

<!-- an indentable JSDoc block re-indents to its context -->
<div>
	<script>
		/**
		 * a
		 */
		fn();
	</script>
</div>

{#if cond}
	<script lang="ts">
		const f: string = `x
y`;
	</script>
{/if}

<svelte:head>
	<script>
		const g = `x
y`;
	</script>
</svelte:head>

<!-- a prettier-ignore statement keeps its interior lines verbatim -->
<div>
	<script>
		// prettier-ignore
		const h = [
		  1,   2,
		      3];
		fn();
	</script>
</div>

<!-- a comment-only body keeps its continuation line -->
<div>
	<script>
		/* a
b */
	</script>
</div>
