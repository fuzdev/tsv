<script>
	// JSDoc type cast comment after comma in chain call - 2 args
	a.b('x', /** @type {T} */ c);

	// 3 args with comment on middle arg
	a.b('x', /** @type {T} */ c, d);

	// Multiple comments on different args
	a.b(/** @type {A} */ a, /** @type {B} */ b);

	// Regular block comment (not JSDoc)
	a.b('x', /* cast */ c);

	// Chain with member call on second arg
	a.b('x', /** @type {T} */ c.d(e));
</script>
