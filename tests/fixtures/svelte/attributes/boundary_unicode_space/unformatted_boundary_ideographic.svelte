<!--
	A JS `\s` character after an attribute's value ENDS the attribute — Svelte's
	`regex_invalid_unquoted_attribute_value` spells that class as `\s`, so every
	non-ASCII member separates too (see unformatted_boundary_nbsp and
	unformatted_boundary_ideographic). Two symptoms, one per element: absorbing it
	before another attribute makes the head unparseable, and absorbing it before
	`>` turns the expression value into a quoted string. The brace-interior
	question is its sibling (see attributes/interior_unicode_space).
-->
<div onclick={handler}　data-attr="value"　hidden></div>
<span onclick={handler}　></span>
