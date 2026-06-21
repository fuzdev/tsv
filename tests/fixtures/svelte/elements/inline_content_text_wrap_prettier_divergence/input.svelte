<!-- Inline element with text content: the opening tag stays intact and the text flows after it, wrapping at spaces; only the closing > dangles. tsv keeps tags attached to text content (text has internal break points at every space, so no >-dangle is needed). Prettier instead pre-breaks the opening tag (<small> -> <small\n>); see output_prettier + README. -->
<div>
	<small>word word word word word word word word word word word word word word word word word word
		word word word word</small
	>
</div>
