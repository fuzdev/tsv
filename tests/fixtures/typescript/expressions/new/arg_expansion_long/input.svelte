<script>
	// Short new (stays inline)
	const a = new A(cond ? () => b : () => c, (x) => fn(x, y));

	// Multi-arg new with ternary + arrow (args expand to separate lines)
	const b = new VeryLongClassName(COND ? () => window.scrollX : () => undefined, (update) =>
		on(window, 'scroll', update)
	);

	// Multi-arg new with object arg (args expand)
	const c = new Proxy(
		{ ...a, b: {} },
		{
			get(target, prop) {
				return get(sources.get(prop));
			},
			set(target, prop, value) {
				set(sources.get(prop), value);
				return Reflect.set(target, prop, value);
			}
		}
	);
</script>
