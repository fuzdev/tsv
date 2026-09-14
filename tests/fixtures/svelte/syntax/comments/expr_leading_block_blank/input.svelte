<!-- An author blank line after a single-line block comment the author broke after, in a
	braced head's head→value gap: the break is the author's, and so is the blank below it. -->
{/* c1 */

expr}
{@html /* c2 */

expr}
{@render /* c3 */

fn()}
{#each list as it}
	{@const y =
		/* c4 */

		it}
	{it}
{/each}
<div
	{.../* c5 */

	expr}
></div>
<input
	bind:value={
		/* c6 */

		val
	}
/>
<div
	data-attr={/* c7 */

	expr}
></div>
<!-- a `bind:` sequence's comma gap, through the sequence's own emitter -->
<input
	bind:value={
		get,
		/* c8 */

		set
	}
/>
<!-- a multi-line block keeps the blank after it too -->
{@html /* c9
c10 */

expr}
