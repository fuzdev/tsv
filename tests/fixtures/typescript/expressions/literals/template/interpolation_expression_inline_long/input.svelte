<script lang="ts">
	// Member expression, compact source - stays inline past printWidth (atomized), matches prettier
	const a = `${obj.aaa.bbb.ccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc}`;

	// Multiple interpolations, compact source - stay inline (atomized), matches prettier
	const b = `${obj.aaa.bbb}__${fn()}__${obj.ccc.ddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd}`;

	// Call chain, compact source - stays inline (atomized), matches prettier
	const c = `${obj.fn().method().anotherrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrr}`;

	// Tagged template, compact source - stays inline (atomized), matches prettier
	const d = tag`text1 ${obj.aaa.bbb.cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc}`;

	// Logical AND (qualifying type) - compact source stays inline (atomized); breaks only with a source newline
	const e = `${aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa && bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb}`;

	// Nullish coalescing (qualifying type) - compact source stays inline (atomized)
	const f = `${aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa ?? bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb}`;

	// TSAsExpression (qualifying type) - compact source stays inline (atomized)
	const g = `${aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa as BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB}`;

	// TSSatisfiesExpression (qualifying type) - compact source stays inline (atomized)
	const h = `${aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa satisfies BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB}`;

	// SequenceExpression (qualifying type) - compact source stays inline (atomized)
	const i = `${(aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa, bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb)}`;
</script>
