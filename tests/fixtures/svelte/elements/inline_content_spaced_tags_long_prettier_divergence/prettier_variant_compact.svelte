<!-- 100 chars - fits, stays inline -->
<div>
	<small>{a} {bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb}</small>
</div>

<!-- 101 chars - overflows; block-style, and the tags pack on the content line -->
<div>
	<small>{a} {bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb}</small
	>
</div>

<!-- 101 chars, text between the tags - the content flows as one fill, so they share a line -->
<div>
	<small>{a} and {cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc}</small
	>
</div>
