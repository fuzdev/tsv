<!-- A leading comment the author left on its own line keeps the value below it —
the value never pulls up onto the comment's closing line. Covers the JS-expression
gaps that share one leading-comment emitter. -->
<div
	class={/* l1
l2 */
	x}
>
	{/* l1
l2 */
	x}
</div>
{#if cond}
	{@const a =
		/* l1
l2 */
		x}
	<!-- Contrast: a comment glued to the value leads it inline -->
	{@const b = /* l1
l2 */ x}
{/if}
