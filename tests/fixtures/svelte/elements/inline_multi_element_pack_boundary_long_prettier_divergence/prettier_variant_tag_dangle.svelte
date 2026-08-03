<!--
	The mid-run pack's exact width boundary inside an INLINE container. Text after the first
	element packs onto its line up to 100 columns; the second element then folds with its
	terminal trailing text on the next line. The two spans pin 100/101: `abcd` fills the first
	line to exactly 100 (`lima` still fits), and one character more (`abcde`) pushes `lima` onto
	the second line, where the element and its tail flow after it. See README.
-->
<span
	>alpha bravo charlie delta echo foxtrot golf hotel india juliet abcd <code>inline1</code> kilo
	lima <code>inline2</code> mike november oscar papa</span
>

<span
	>alpha bravo charlie delta echo foxtrot golf hotel india juliet abcde <code>inline1</code> kilo
	lima <code>inline2</code> mike november oscar papa</span
>
