<script lang="ts">
	// Arrow parameter defaults containing an unbalanced paren inside a literal —
	// the regex/template delimits the paren, so it is not a grouping.

	// Regex, escaped close paren
	const a = (x = /\)/) => x;

	// Regex, escaped open paren, async arrow
	const b = async (x = /\(/) => x;

	// Regex, close paren in a char class, constrained generic arrow
	const c = <T extends X>(x = /[)]/) => x;

	// Template literal containing a close paren
	const d = (x = `)`) => x;

	// Regex default inside a destructuring object pattern
	const e = ({ x = /\)/ }) => x;
</script>
