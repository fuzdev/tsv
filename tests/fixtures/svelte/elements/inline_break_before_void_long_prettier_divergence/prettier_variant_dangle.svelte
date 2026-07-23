<!--
	Void / self-closing parity for the break-before rule: a void element (<img />) preceded
	by same-line text moves to its own line rather than dangling its opening tag; with no
	content it simply sits on the fresh line. Prettier keeps the opening tag on the text
	line and wraps its attributes — see prettier_variant_dangle.svelte.
-->
<p>
	word1 word2 word3 word4 word5 word6 word7 word8 word9 word10 word11 word12 word13 <img
		src="/a"
		alt="x"
	/>
</p>
