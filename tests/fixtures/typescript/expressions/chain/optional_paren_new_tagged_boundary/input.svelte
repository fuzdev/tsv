<script>
	// An optional chain can't be the callee of `new` or the tag of a tagged template
	// (per spec), so these parens are required: stripping them is a syntax error.
	// (The boundary-dependent member/call/computed cases are in optional_paren_boundary.)

	// `new` callee — optional chain base needs parens
	new (a?.b)();
	new (a?.b())();
	new (a?.())();

	// tagged template tag — optional chain base needs parens
	(a?.b)`tpl`;
	(a?.b())`tpl`;
	(a?.())`tpl`;
</script>
