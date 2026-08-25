<!--
	The deferred separator in front of a run-ending sibling — a comment, a {@debug} — is that
	follower's own per-width wrap, exactly as before an element: when the element overflows and
	goes block-style, the content line is only 97 wide, so the follower stays on it. Prettier
	breaks that space with the container instead. The width boundary of the hug itself is in
	inline_sibling_space_before_bounding.
-->

<!-- exactly 100 chars — inline, and the separator is the rendered space -->
<p>aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa text1 <Comp /> <!-- c -->text2</p>

<!-- 101 chars — the element breaks block-style, and the follower keeps its space on the content line -->
<p>
	aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa text1 <Comp /> <!-- c -->text2
</p>

<!-- the `{@debug}` follower is the same run-ending kind, at the same boundary -->
<p>
	aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa text1 <Comp /> {@debug a}text2
</p>
