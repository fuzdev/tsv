<!--
	`{@attach}` and `{...spread}` are the markers an attribute POSITION accepts. Every
	other `{`-marker leaves an interior that is not an identifier, and the shorthand
	reader rejects it — the input_invalid_* files carry those spellings, the non-attach
	`{@…}` included (it is the attach reader that rejects that one, on its keyword).
-->
<div {@attach fn} {...a} {b}></div>
