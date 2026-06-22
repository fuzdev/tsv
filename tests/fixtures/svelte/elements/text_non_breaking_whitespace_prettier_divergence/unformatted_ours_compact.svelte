<!-- Normal spaces are break points: long inline text fill-wraps across lines -->
<span>word1 word2 word3 word4 word5 word6 word7 word8 word9 word10 word11 word12 word13 word14 word15 word16 word17 word18</span>

<!-- Non-breaking spaces (U+00A0) are not break points: long text stays on one line -->
<span>word1 word2 word3 word4 word5 word6 word7 word8 word9 word10 word11 word12 word13 word14 word15 word16 word17 word18</span>

<!-- Narrow non-breaking space (U+202F) is preserved verbatim -->
<span>Prix : 32 €</span>

<!-- Leading non-breaking space is preserved, not turned into a regular space -->
<span> word1 word2</span>

<!-- Trailing non-breaking space is preserved, not turned into a regular space -->
<span>word1 word2 </span>

<!-- Root-level text: non-breaking spaces preserved, regular spaces collapse -->
foo bar baz qux
