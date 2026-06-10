<script>
	// IIFE callee keeps its parens even when the call result is member-accessed
	const a = (() => 1)().p;
	const b = (async () => 1)().p;
	const c = (function () {})().p;
	const d = (() => fn())().m();
	// a bare IIFE (no trailing member) also keeps parens
	const e = (() => 1)();
	// long body: the parens hug the function, they do not break onto their own line
	void (async () => {
		const value = await loadSomethingWithARatherLongNameToForceThisToWrap();
		register(value);
	})().catch(onError);
</script>
