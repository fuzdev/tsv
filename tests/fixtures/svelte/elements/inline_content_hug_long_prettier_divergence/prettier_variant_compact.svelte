<!-- 100 chars -->
<div>
	<small>{a !== 1 ? 's' : ''}{b !== 1 ? 's' : ''}{ccccccccccccccccccccccccccccccccccccccccc}</small>
</div>

<!-- 101 chars - overflows, lays out block-style (both tags intact, content on its own indented line) -->
<div>
	<small>{a !== 1 ? 's' : ''}{b !== 1 ? 's' : ''}{cccccccccccccccccccccccccccccccccccccccccc}</small
	>
</div>

<!-- 102 chars - block-style; content still fits unbroken once on its own indented line -->
<div>
	<small
		>{a !== 1 ? 's' : ''}{b !== 1 ? 's' : ''}{ccccccccccccccccccccccccccccccccccccccccccc}</small
	>
</div>

<!-- Logical chain - block-style; the && chain fits unbroken on its own indented line (prettier breaks it one-per-line) -->
<div>
	<small
		>{aaa &&
			bbb &&
			ccc &&
			ddd &&
			eee &&
			fff &&
			ggg &&
			hhh &&
			iii &&
			jjj &&
			kkk &&
			lll &&
			mmm}</small
	>
</div>

<!-- Binary chain - block-style; the + chain fits unbroken on its own indented line (prettier breaks it one-per-line) -->
<div>
	<small
		>{arr[0] +
			arr[1] +
			arr[2] +
			arr[3] +
			arr[4] +
			arr[5] +
			arr[6] +
			arr[7] +
			arr[8] +
			arr[9]}</small
	>
</div>
