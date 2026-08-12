<!--
	The travel rule from the other side of the boundary. Here the whitespace in front of the welded
	run is the run's OWN leading boundary — the previous sibling is an inline element or tag, so no
	preceding text fill holds it. The run travels whole at that boundary however wide it is and
	whatever heads it. Cases: a lone glued word (`(`) heading the run at @100 (fits, stays) and
	@101 (travels); the same @101 before an expression TAG sibling; the same @101 with a TWO-word
	head (the word count of the head cannot change where the run breaks); a run far too wide to
	pack, which still fits on its own line; and a run whose element is wider than a whole line,
	which travels anyway and then lays out block-style with both tags intact. Prettier dangles the
	tag delimiters instead — from this authoring on the last three, from the compact one on all —
	see output_prettier.svelte and prettier_variant_dangle.svelte.
-->
<p>
	aaaa bbbb cccc dddd eeee ffff gggg hhhh iiii jjjj kkk <code>pppp</code> (<a href="/a/b">qqqq</a>):
</p>
<p>
	aaaa bbbb cccc dddd eeee ffff gggg hhhh iiii jjjj kkkk <code>pppp</code> (<a href="/a/b">qqqq</a
	>):
</p>
<p>
	aaaa bbbb cccc dddd eeee ffff gggg hhhh iiii jjjj kkkk llll mmmmm {expr} (<a href="/a/b">qqqq</a
	>):
</p>
<p>
	aaaa bbbb cccc dddd eeee ffff gggg hhhh iiii jjjjjj <code>pppp</code> zz (<a href="/a/b">qqqq</a
	>):
</p>
<p>
	aaaa bbbb cccc dddd eeee ffff gggg hhhh iiii jjjj kkkk <code>pppp</code> (<a
		href="/a/b/cccccccccccccccccccccccccccccccccccc">qqqq</a
	>):
</p>
<p>
	aaaa bbbb cccc dddd eeee ffff gggg hhhh iiii jjjj kkkk <code>pppp</code> (<a
		href="/a/b/cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc"
		>qqqq</a
	>):
</p>
