<script>
	// two-level nested cast
	const a = /** @type {A} */ (/** @type {B} */ (expr));

	// nested cast with member access inside
	const b = /** @type {A} */ (/** @type {B} */ (expr.prop));

	// three-level nested cast
	const c = /** @type {A} */ (/** @type {B} */ (/** @type {C} */ (expr)));

	// nested cast as a call argument
	const d = fn(/** @type {A} */ (/** @type {B} */ (expr)));
</script>
