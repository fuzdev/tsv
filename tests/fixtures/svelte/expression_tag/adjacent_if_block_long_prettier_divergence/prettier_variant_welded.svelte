<script>
	let aaaaaa, bbbbbbbb, aaaaa, bbbbbbbbb, aaaaaaaaaaaaaaaaaa, bbbbbbbbbbbbbbbbbb;
</script>

<!-- 100 chars - stays on one line (at boundary) -->
{fn(aaaaaa || bbbbbbbb)}{#if aaaaaa && bbbbbbbb && aaaaaa !== bbbbbbbb}, updated {fn(bbbbbbbb)}{/if}

<!-- 101 chars - inner fn() args break (exceeds print_width) -->
{fn(aaaaaa || bbbbbbbb)}{#if aaaaaa && bbbbbbbb && aaaaaa !== bbbbbbbb}, updated {fn(
		bbbbbbbbb
	)}{/if}

<!-- 176 chars - first fn() args break, text content also breaks -->
{fn(
	aaaaaaaaaaaaaaaaaa || bbbbbbbbbbbbbbbbbb
)}{#if aaaaaaaaaaaaaaaaaa && bbbbbbbbbbbbbbbbbb && aaaaaaaaaaaaaaaaaa !== bbbbbbbbbbbbbbbbbb},
	updated {fn(bbbbbbbbbbbbbbbbbb)}{/if}
