<script>
	// assignment
	const a = /** @type {A} */ document.activeElement;
	const b = /** @type {A} */ b.parentNode;
	const c = /** @satisfies {A} */ expr;

	// reassignment
	let d;
	d = /** @type {A} */ expr;

	// return
	function fn1() {
		return /** @type {A} */ expr;
	}

	// call argument
	const e = fn(/** @type {A} */ {});
	const f = fn(/** @type {A} */ expr);

	// new expression with expand-last arg
	new A(/** @type {any} */ a, {});
	new A(/** @type {any} */ a, []);
	new A(a, /** @type {any} */ b, {});

	// default parameter value
	function fn2(a = /** @type {A} */ b) {}
	function fn3(a, b = /** @type {A} */ c) {}
	const fn4 = (a = /** @type {A} */ b) => a;
	function fn5({a = /** @type {A} */ b} = {}) {}
</script>
