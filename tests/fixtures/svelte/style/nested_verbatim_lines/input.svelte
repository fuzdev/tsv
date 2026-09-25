<!-- a multi-line comment keeps its continuation lines, trailing spaces included -->
<div>
	<style>
		/* a   
b */
		.class {
			color: red;
		}
	</style>
</div>

<!-- a line-continued string keeps its continuation line -->
<div>
	<section>
		<style>
			.class {
				content: 'a\
b';
			}
		</style>
	</section>
</div>

<!-- an unknown at-rule prelude keeps its newline -->
{#if cond}
	<style>
		@a b
c {
		}
	</style>
{/if}

<svelte:head>
	<style>
		/* a
b */
	</style>
</svelte:head>

<!-- comment interiors are verbatim: star-aligned and deeper-column lines keep their columns -->
<div>
	<style>
		/*
		 * a
		 */
		/* a
					b */
		.class {
			color: red;
		}
	</style>
</div>

<!-- a prettier-ignore rule keeps its interior lines verbatim -->
<div>
	<style>
		/* prettier-ignore */
		.class1   {
		  color:red;
		      }
		.class2 {
			color: red;
		}
	</style>
</div>
