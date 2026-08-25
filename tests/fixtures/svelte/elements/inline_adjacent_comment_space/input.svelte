<!--
	A whitespace-only separator between a non-text sibling and a COMMENT is render content: the
	comment itself renders nothing, so that space is the only thing holding the two runs apart.
	It survives at every predecessor kind, and `{@debug}` is the same follower — it renders
	nothing and ends the inline run the same way, so it takes the same separator.
-->
<p>text1 <span>x</span> <!-- c -->text2</p>

<!-- a TAG predecessor: what carries the space is the separator, not the element before it -->
<p>text1 {expr} <!-- c -->text2</p>

<!-- the `{@debug}` follower, in the identical position -->
<p>text1 <span>x</span> {@debug a}text2</p>

<!--
	null control: NO separator was authored, so none is emitted — the glued form stays glued.
	It varies the same dimension (is there a separator at this boundary?), so an implementation
	that always emitted a space before a comment would pass every case above and fail here.
-->
<p>text1 <span>x</span><!-- c -->text2</p>

<!--
	control: multiline STRUCTURALLY (a block child), authored with a NEWLINE before the comment,
	which is held — the space spelling stays a space in a multiline container too
	(inline_sibling_space_before_bounding)
-->
<div>
	text1 <span>x</span>
	<!-- c -->text2
	<div>block1</div>
</div>
