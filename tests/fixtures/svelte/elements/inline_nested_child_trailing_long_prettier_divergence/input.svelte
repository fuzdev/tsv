<!-- Fits: the nested inline child's line is exactly 100, so it stays inline; the
     trailing text (authored on its own line) keeps its own line. -->
<span>
	<span class="class1 class2 class3 class4 class5 class6 class7abc" style:width="6ch">{expr}</span>
	text
</span>

<!-- Breaks: one char longer (101), so the nested child wraps; the no-attr parent keeps its
     open tag hugging the child and the trailing text keeps its own line. -->
<span>
	<span class="class1 class2 class3 class4 class5 class6 class7abcd" style:width="6ch">{expr}</span>
	text
</span>
