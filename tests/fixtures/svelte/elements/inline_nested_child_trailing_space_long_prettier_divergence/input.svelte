<!-- An inline <span> wrapping a wide inline child <span> (its open tag overflows) plus trailing
     text. tsv drops the child below the parent's open tag and puts the trailing text on its own
     line. Authoring the boundary before the trailing text as a space reaches this same form
     (unformatted_ours_compact); prettier instead hugs the trailing text after the child's closing
     tag (prettier_variant_hug). See README. -->
<p>
	<span
		><span class="class1 class2 class3 class4 class5 class6 class7" style:width="6ch"
			>{cond ? 'aaaa' : 'bbbb'}</span
		>
		text</span
	>
</p>
