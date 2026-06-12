<!-- Text-only content across source lines collapses to inline when it fits -->
<p>text1 text2</p>

<!-- Multiple words across lines -->
<p>text1 text2 text3 text4</p>

<!-- Different block elements -->
<h1>text1 text2</h1>

<!-- Nested in container -->
<div>
	<p>text1 text2</p>
	<h2>text1 text2</h2>
</div>

<!-- Text-only, too long for inline - stays multiline (control case) -->
<p>
	text1 text2 text3 text4 text5 text6 text7 text8 text9 text10 text11 text12 text13 text14 text15
	text16 text17.
</p>

<!-- Text with inline element child - stays multiline (control case) -->
<p>
	text1
	<span>text2</span>
</p>
