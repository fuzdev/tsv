<!--
	The deferred separator in front of a run-ending sibling is a BARE collapsible break, resolved
	by the container's own group: a space while the whole element fits, a newline the moment it
	does not. The 101-char cases are the discriminator — their content line is only 97 wide, so a
	separator measured on its OWN width would keep the sibling on it, and the next pass, reading
	that break back, would move it anyway.
-->

<!-- exactly 100 chars — inline, and the separator is the rendered space -->
<p>aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa text1 <Comp /> <!-- c -->text2</p>

<!-- 101 chars — the element breaks, and the separator breaks with it -->
<p>
	aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa text1 <Comp />
	<!-- c -->text2
</p>

<!-- the `{@debug}` follower is the same run-ending kind, at the same boundary -->
<p>
	aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa text1 <Comp />
	{@debug a}text2
</p>
