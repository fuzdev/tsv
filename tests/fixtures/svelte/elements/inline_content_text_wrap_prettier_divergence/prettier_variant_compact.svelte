<!-- An inline element (`<small>`) whose text content overflows print width lays out block-style:
     both tags stay intact and the content moves to its own indented line, exactly like a block
     element. Content that fits stays inline (`<small>short text</small>`). Content-boundary
     whitespace is render-free under Svelte 5, so tsv places it freely. Prettier instead pre-breaks
     the opening tag and dangles the closing `>`; see output_prettier + README. -->
<div>
	<small>short text</small>
	<small
		>word word word word word word word word word word word word word word word word word word word</small
	>
</div>
