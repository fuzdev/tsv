<script lang="ts">
	// Arrow parameter defaults ending in a division — the scan for the `)` that closes
	// the parameter list reads each `/` as the operator it is, not as a regex's opener.

	// After a type-argument list's `>`
	const a = (x = f<T> / 2) => x / 2;

	// After a postfix non-null `!`
	const b = (x = c! / 2) => x / 2;

	// After a numeric literal (its trailing `.` in the variant)
	const d = (x = 1 / 2) => x / 2;

	// After a non-ASCII identifier character
	const e = (x = é / 2) => x / 2;

	// After a regex literal with no flags
	const f = (x = /a/ / 2) => x / 2;
	// A prefix `!` after a statement's `)` opens the regex itself
	const g = (
		x = () => {
			if (c) !/[)]/.test(d);
		}
	) => x;
	// A statement header's `)` ends no operand, so the `<` after it is an assertion
	const h = (
		x = () => {
			if (c) <RegExp>/[)]/;
		}
	) => x;
	// A block comment spanning lines is a line break: the `!` after it starts a statement
	const i = (
		x = () => {
			b /* c
			 */!/[)]/.test(d);
		}
	) => x;
	// A paren in a regex in a statement header's condition is no header paren
	const j = (
		x = () => {
			if (/\)/.test(s)) !/[)]/.test(d);
			if (/\(/.test(s)) !/[)]/.test(d);
			if (/\)/.test(s)) <RegExp>/[)]/;
		}
	) => x;
</script>
