<script lang="ts">
	let cond;
	let expr;
</script>

{#if cond}
	<!-- an own-line comment after the pattern's last entry, with an annotation after it -->
	{@const {
		a1
		// c1
	}: T = expr}

	<!-- an own-line block comment, and an array pattern -->
	{@const {
		a2
		/* c2 */
	}: T = expr}

	{@const [
		a3
		// c3
	]: T = expr}

	<!-- nested: the comment closes an inner pattern -->
	{@const {
		a4: {
			b4
			// c4
		}
	}: T = expr}

	<!-- the same-line control: the comment trails the entry -->
	{@const {
		a5 // c5
	}: T = expr}

	<!-- the no-annotation control -->
	{@const {
		a6
		// c6
	} = expr}
{/if}
