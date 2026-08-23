<script lang="ts">
	// A unicode escape is an identifier character, so an escaped name is a single
	// arrow parameter. Prettier decodes the escape and parenthesizes the
	// parameter, so the frozen rows below are where the escaped spelling survives.
	// prettier-ignore
	const a1 = \u0078 => 1;
	// prettier-ignore
	const a2 = \u{78}y => 1;
	// prettier-ignore
	const a3 = x\u0078 => 1;

	// The decoded, parenthesized spellings those normalize to.
	const b1 = \u0078 => 1;
	const b2 = \u{78}y => 1;
	const b3 = x\u0078 => 1;
</script>
