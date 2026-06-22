<script lang="ts">
	// Inner call breaks its args; single trailing member hugs the final `)` (corpus: cell_audit.db.test.ts)
	assert(
		(
			await call(
				applicationObjectLong,
				specificationObjectLong,
				{ aaa: parentNodeId, bbb: childNodeId },
				handlerObjectLong
			)
		).ok
	);

	// Parenthesized cast base hangs across lines; single trailing member hugs the `)`
	const flag = (
		call(
			applicationObject,
			specificationObject,
			{ a: parentNodeId, b: childNodeId },
			handler
		) as Foo
	).ok;

	// Boundary: single-line base, total exactly 100 - member stays inline on the same line
	const fit = (objectExpr as AbcDefGhiJklMnoPqr).propertyNameAaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa;

	// Boundary: single-line base, total 101 - member drops to its own indented line
	const brk = (objectExpr as AbcDefGhiJklMnoPqr)
		.propertyNameAaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa;
</script>
