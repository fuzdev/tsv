<!-- inline element -->
<span>
	<style>
		/* a
b */
		.class {
			color: red;
		}
	</style>
</span>

<!-- component -->
<Comp>
	<style>
		/* a
b */
		.class {
			color: red;
		}
	</style>
</Comp>

<!-- snippet -->
{#snippet fn()}
	<style>
		/* a
b */
		.class {
			color: red;
		}
	</style>
{/snippet}

<!-- each body and else body -->
{#each items as item}
	<style>
		/* a
b */
		.class {
			color: red;
		}
	</style>
{:else}
	<style>
		/* a
b */
		.class {
			color: red;
		}
	</style>
{/each}

<!-- await pending, then and catch bodies -->
{#await promise}
	<style>
		/* a
b */
		.class {
			color: red;
		}
	</style>
{:then value}
	<style>
		/* a
b */
		.class {
			color: red;
		}
	</style>
{:catch error}
	<style>
		/* a
b */
		.class {
			color: red;
		}
	</style>
{/await}

<!-- if, else if and else bodies -->
{#if a}
	<style>
		/* a
b */
		.class {
			color: red;
		}
	</style>
{:else if b}
	<style>
		/* a
b */
		.class {
			color: red;
		}
	</style>
{:else}
	<style>
		/* a
b */
		.class {
			color: red;
		}
	</style>
{/if}

<!-- key -->
{#key expr}
	<style>
		/* a
b */
		.class {
			color: red;
		}
	</style>
{/key}

<!-- svelte:boundary -->
<svelte:boundary>
	<style>
		/* a
b */
		.class {
			color: red;
		}
	</style>
</svelte:boundary>

<!-- svelte:element -->
<svelte:element this="div">
	<style>
		/* a
b */
		.class {
			color: red;
		}
	</style>
</svelte:element>

<!-- svelte:head -->
<svelte:head>
	<style>
		/* a
b */
		.class {
			color: red;
		}
	</style>
</svelte:head>

<!-- nested inline elements -->
<a href="x">
	<b>
		<style>
			/* a
b */
			.class {
				color: red;
			}
		</style>
	</b>
</a>

<!-- label with text -->
<label>
	text
	<style>
		/* a
b */
		.class {
			color: red;
		}
	</style>
</label>

<!-- table cell -->
<table>
	<tbody>
		<tr>
			<td>
				<style>
					/* a
b */
					.class {
						color: red;
					}
				</style>
			</td>
		</tr>
	</tbody>
</table>

<!-- if inside an inline element inside a block element -->
<div>
	<span>
		{#if a}
			<style>
				/* a
b */
				.class {
					color: red;
				}
			</style>
		{/if}
	</span>
</div>

<!-- blocks nested in blocks -->
{#each items as item}
	{#if a}
		{#key expr}
			<style>
				/* a
b */
				.class {
					color: red;
				}
			</style>
		{/key}
	{/if}
{/each}

<!-- an element whose attributes wrap -->
<div
	data-attr1="value1-aaaaaaaaaaaaaaaaaaaaaaaaaaaa"
	data-attr2="value2-bbbbbbbbbbbbbbbbbbbbbbbbbbbb"
	data-attr3="value3"
>
	<style>
		/* a
b */
		.class {
			color: red;
		}
	</style>
</div>
