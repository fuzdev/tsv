<script>
	// cast the whole member expression
	const a = /** @type {A} */ (x.y);

	// cast only the base object, then access a property
	const b = /** @type {A} */ (x).y;

	// cast the whole member, then continue the chain
	const c = /** @type {A} */ (x.y).z;

	// cast the base, then a longer chain
	const d = /** @type {A} */ (x).y.z;

	// cast a call expression
	const e = /** @type {A} */ (fn(x));

	// cast the callee base, then call
	const f = /** @type {A} */ (fn).call(x);

	// cast a computed member
	const g = /** @type {A} */ (x[y]);
</script>
