<!--
	Block-style: an inline <span> wraps a wide inline child <span> whose open tag overflows, plus
	trailing text. Both tags stay intact; content goes on its own indented lines. The trailing text
	hugs the child's `</span>` line whichever way its boundary is authored — that boundary is
	render-free, so tsv converges the newline form (prettier_variant_ownline) to the hug while
	prettier keeps it stable. A compact authoring dangles under prettier (prettier_variant_compact)
	but tsv likewise converges it to the hug form. See README.
-->
<p>
	<span
		><span class="class1 class2 class3 class4 class5 class6 class7" style:width="6ch"
			>{cond ? 'aaaa' : 'bbbb'}</span
		> text</span
	>
</p>
