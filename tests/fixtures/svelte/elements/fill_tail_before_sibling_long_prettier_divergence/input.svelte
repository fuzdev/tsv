<!--
	Tail text followed by an inline sibling: the fill carries BOTH a leading and a trailing
	separator, so its trailing `line` lands in the last-item slot. The break must still hold
	the line <= 100 — measuring the leading `line` alone made this non-idempotent.
-->
<li>
	aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa ~{xxxxxxxxxx}
	cc <b>x</b>
</li>
