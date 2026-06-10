<!-- Non-breaking space (U+00A0) as the entire text node between inline elements is preserved, not collapsed to a regular space -->
<span>inline1</span> <span>inline2</span>

<!-- Narrow non-breaking space (U+202F) separator is likewise preserved -->
<span>inline1</span> <span>inline2</span>

<!-- Non-breaking space separator between inline components is preserved -->
<Comp1 /> <Comp2 />

<!-- Regular spaces around a non-breaking space collapse to single boundary spaces; the nbsp stays as content -->
<span>inline1</span>       <span>inline2</span>

<!-- Nested inside a block element: the nbsp separator between inline children is preserved -->
<div><span>inline1</span> <span>inline2</span></div>
