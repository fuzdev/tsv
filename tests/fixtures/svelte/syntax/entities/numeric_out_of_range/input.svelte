<!--
	A numeric character reference with no character to decode to becomes NUL: a surrogate
	half, a code past the last code point Unicode defines, and — the same answer — a digit
	run too long to fit a 32-bit value. The last reference is the contrast, a code point
	that decodes.
-->
<p>unrepresentable: &#xD800; &#1114112;</p>
<p>overflowing: &#4294967296; &#99999999999999; &#xFFFFFFFFFF;</p>
<p>representable: &#x2FFFF;</p>
