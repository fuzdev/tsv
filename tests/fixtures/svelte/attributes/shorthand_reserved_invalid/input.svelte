<!--
	A shorthand attribute's interior goes through Svelte's `read_identifier`, which
	rejects a reserved word. These near-misses are NOT on that list and stay valid;
	the input_invalid_* files carry the rejected spellings.
-->
<div {of} {async} {get} {set} {from} {as} {undefined}></div>
