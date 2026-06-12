<script lang="ts">
	// simple nested cast in assignment
	const a = /** @type {A} */ /** @type {B} */ expr;

	// nested cast with member access
	const b = /** @type {A} */ /** @type {B} */ expr.prop;

	// nested cast as call argument
	const c = fn(/** @type {A} */ /** @type {B} */ expr);

	// nested cast as second call argument
	const d = fn(a, /** @type {A} */ /** @type {B} */ expr);

	// nested cast in return
	function fn1() {
		return /** @type {A} */ /** @type {B} */ expr;
	}

	// nested cast in assignment rhs
	let e;
	e = /** @type {A} */ /** @type {B} */ expr;

	// deeply nested (three levels)
	const f = /** @type {A} */ /** @type {B} */ /** @type {C} */ expr;

	// expression statement (standalone)
	/** @type {A} */ /** @type {B} */ expr;

	// expression statement with member access
	/** @type {A} */ /** @type {B} */ expr.prop;
</script>
