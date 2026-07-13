<script>
	let aaaaaa, bbbbbbbb, aaaaa, bbbbbbbbb, aaaaaaaaaaaaaaaaaa, bbbbbbbbbbbbbbbbbb;
</script>

<!-- 100 chars - stays on one line (at boundary) -->
{fn(aaaaaa || bbbbbbbb)}{#if aaaaaa && bbbbbbbb && aaaaaa !== bbbbbbbb}, updated {fn(bbbbbbbb)}{/if}

<!-- 101 chars - the block body expands onto its own line (exceeds print_width) -->
{fn(aaaaaa || bbbbbbbb)}{#if aaaaaa && bbbbbbbb && aaaaaa !== bbbbbbbb}
	, updated {fn(bbbbbbbbb)}
{/if}

<!-- 176 chars - the first fn() args break, and the block body expands -->
{fn(
	aaaaaaaaaaaaaaaaaa || bbbbbbbbbbbbbbbbbb
)}{#if aaaaaaaaaaaaaaaaaa && bbbbbbbbbbbbbbbbbb && aaaaaaaaaaaaaaaaaa !== bbbbbbbbbbbbbbbbbb}
	, updated {fn(bbbbbbbbbbbbbbbbbb)}
{/if}
