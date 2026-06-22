<!-- Block-style: an inline <span> wraps a wide inline child <span> whose open tag overflows, plus
     trailing text. Both tags stay intact; content goes on its own indented lines. The trailing-text
     boundary is authoring-dependent for now: a space hugs the text onto the child's `</span>` line
     (input.svelte), a newline keeps it on its own line (variant_ownline.svelte). Both are stable
     under tsv AND prettier. A compact authoring dangles under prettier (prettier_variant_compact)
     but tsv converges it to the hug form. Converging the two boundary forms is a pending follow-up.
     See README. -->
<p>
	<span
		><span class="class1 class2 class3 class4 class5 class6 class7" style:width="6ch"
			>{cond ? 'aaaa' : 'bbbb'}</span
		> text</span
	>
</p>
