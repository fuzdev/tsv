<!--
	An inline element glued to text on both sides, whose own attribute forces the opening tag to
	break, lays out its content block-style like its spaced twin. An attribute authored across
	lines that prints flat forces nothing: the element stays on one line.
-->

<!-- a block-bodied handler -->
<p>
	text1<button
		onclick={() => {
			a += 1;
			fn(a);
		}}>inline</button
	>text2
</p>

<!-- a quoted value spanning lines -->
<p>
	text1<a
		href="/a"
		data-attr="value1
value2">inline1 inline2</a
	>text2
</p>

<!-- a template literal spanning lines -->
<p>
	text1<span
		title={`value1
value2`}>inline</span
	>text2
</p>

<!-- a block comment spanning lines -->
<p>
	text1<span
		data-attr={/* c1
c2 */ a}>inline</span
	>text2
</p>

<!-- an element child -->
<p>
	text1<button
		onclick={() => {
			a += 1;
			fn(a);
		}}><i>inline</i></button
	>text2
</p>

<!-- an expression child -->
<p>
	text1<button
		onclick={() => {
			a += 1;
			fn(a);
		}}>{expr}</button
	>text2
</p>

<!-- a component -->
<p>
	text1<Comp
		onclick={() => {
			a += 1;
			fn(a);
		}}>inline</Comp
	>text2
</p>

<!-- a svelte:element -->
<p>
	text1<svelte:element
		this="span"
		onclick={() => {
			a += 1;
			fn(a);
		}}>inline</svelte:element
	>text2
</p>

<!-- an inline parent -->
<p>
	<span>
		text1<button
			onclick={() => {
				a += 1;
				fn(a);
			}}>inline</button
		>text2
	</span>
</p>

<!-- content that wraps -->
<p>
	text1<button
		onclick={() => {
			a += 1;
			fn(a);
		}}
		>inline1 inline2 inline3 inline4 inline5 inline6 inline7 inline8 inline9 inline10 inline11
		inline12</button
	>text2
</p>

<!-- at the root -->
text1<button
	onclick={() => {
		a += 1;
		fn(a);
	}}>inline</button
>text2

<!-- an attribute authored across lines that prints flat, at the root -->
text1<button onclick={() => fn(a)}>inline</button>text2

<!-- an attribute authored across lines that prints flat, in a broken parent -->
<p>
	text1<span data-attr={a + b}>inline</span>text2
</p>
