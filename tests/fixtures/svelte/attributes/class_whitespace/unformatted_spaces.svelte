<!-- Multiple spaces collapsed to single space -->
<div class="a   b    c"></div>

<!-- Trailing whitespace trimmed -->
<div class="a b c   "></div>

<!-- Leading whitespace after quote preserved, trailing trimmed -->
<div class="  a b c  "></div>

<!-- Newlines preserved as literal lines -->
<div
	class="a b
c d"
></div>

<!-- Whitespace around newlines trimmed -->
<div
	class="a   b
   c   d   "
></div>

<!-- Component prop NOT normalized (PascalCase) -->
<Comp class="a   b    c   " />

<!-- Non-class attribute NOT normalized -->
<div data-attr="a   b    c   "></div>
