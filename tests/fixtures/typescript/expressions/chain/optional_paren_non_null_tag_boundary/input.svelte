<script lang="ts">
	// A non-null assertion sealing a parenthesized optional chain keeps the parens as a
	// tagged-template tag: `(a?.b)!`tpl`` tags the asserted result. The bare form
	// `a?.b!`tpl`` is a syntax error (an optional chain can't be a template tag), so the
	// parens are required. (The non-`!` tag cases are in optional_paren_new_tagged_boundary.)
	(a?.b)!`tpl`;
	(a?.())!`tpl`;
</script>
