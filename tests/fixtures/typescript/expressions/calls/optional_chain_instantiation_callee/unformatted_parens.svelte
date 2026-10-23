<script lang="ts">
	// an instantiation callee keeps its type arguments ahead of the optional call's `?.`
	(T<U>)?.(a).b;
	x = (a.b<U>)?.(c).d;
	const v = T<U, V>?.(a, b).c.d();
	T<U>?.(a)?.(b);
	fn(T<U>?.(a).b);
	T<U>?.(a)[0];
	T<U>?.(a)!.b;
	T<U>?.(a).b?.c();
	someObjectName<TypeArgument>?.(argumentValue)
		.methodOne(x)
		.methodTwo(y)
		.methodThree(z)
		.methodFour(w);

	// control: with no arguments the `?.` fuses into the `?.(` opener
	T<U>?.().b;
</script>

<p>{T<U>?.(a).b}</p>
