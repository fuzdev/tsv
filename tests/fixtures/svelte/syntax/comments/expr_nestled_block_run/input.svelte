<!--
	Two block comments the author left byte-adjacent, both indentable, are ONE comment in
	every JS island a template holds, exactly as in a `<script>` body
	(typescript/syntax/comments/nestled_block_run). CSS is not one of them.
-->

<!-- an expression tag -->
{/** a
 *//** b
 */ expr}

<!-- an attribute value -->
<div
	data-attr={/** a
	 *//** b
	 */ expr}
>
	text1
</div>

<!-- an event handler -->
<div
	onclick={/** a
	 *//** b
	 */ fn}
>
	text1
</div>

<!-- a spread -->
<div
	{.../** a
	 *//** b
	 */ props}
>
	text1
</div>

<!-- an `{@html}` tag -->
{@html /** a
 *//** b
 */ expr}

<!-- a `{@const}` value -->
{#if cond}
	{@const x =
		/** a
		 *//** b
		 */ expr}
	<div>block1</div>
{/if}

<!-- control - one byte of gap and the pair is two comments -->
{/** a
 */ /** b
 */ expr}

<style>
	/* control - CSS is not a JS island: prettier merges nestled comments in its JS parse
	 * alone, so a CSS pair stays two comments */
	/** a
	 */ /** b
	 */
	.class {
		color: red;
	}
</style>
