<!-- Block-style: an inline <span> wraps a wide inline child <span> (its open tag overflows) plus
     trailing text; both tags stay intact and the content goes on its own indented lines. The
     trailing-text boundary is authoring-dependent for now: a newline boundary keeps the text on its
     own line (this input), a space boundary hugs it onto the child's `</span>` line (variant_hug).
     Both forms are stable under tsv AND prettier. Converging the two (always hug, reflowing the
     newline as render-free) is a deliberate pending follow-up. See README. -->
<p>
	<span>
		<span class="class1 class2 class3 class4 class5 class6 class7" style:width="6ch">
			{cond ? 'aaaa' : 'bbbb'}
		</span> text
	</span>
</p>
