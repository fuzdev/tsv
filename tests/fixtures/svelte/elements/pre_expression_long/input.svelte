<!-- 100 chars - doesn't break (at print_width boundary) -->
<pre class="short">{fn(aaaaaaaaaaaaaaaa, bbbbbbbbbbbbbbbb, cccccccccccccccc, ddddddddddddddd)}</pre>

<!-- 101 chars - BREAKS: expression args get 2 tabs (1 for <pre>, 1 for call nesting) -->
<pre class="short">{fn(
		aaaaaaaaaaaaaaaa,
		bbbbbbbbbbbbbbbb,
		cccccccccccccccc,
		dddddddddddddddd,
	)}</pre>
