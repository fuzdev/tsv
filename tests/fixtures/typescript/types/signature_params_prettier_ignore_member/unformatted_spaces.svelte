<script lang="ts">
	// a method signature's parameters
	type A = {
		m(
			// prettier-ignore
			p:   {x:   1},
			q:   U
		): void;
	};

	// call and construct signatures
	type B = {
		(
			// prettier-ignore
			p:   {x:   1},
				q:   U
		): void;
		new (
			// prettier-ignore
			p:   {x:   1}
		): void;
	};

	// a function type and a constructor type
	type C = (
		// prettier-ignore
		p:   {x:   1},
		q:   U
	) => void;

	type D = new (
		// prettier-ignore
		p:   {x:   1},
			q:   U
	) => void;

	// an interface method signature, directive between parameters
	interface E {
		m(
			p:   T,
			// prettier-ignore
			q:   {x:   1}
		): void;
	}

	// an own-line block comment behaves identically
	type F = (
		/* prettier-ignore */
		p:   {x:   1},
		q:   U
	) => void;

	// a bodyless declaration takes the signature path
	declare function g(
		// prettier-ignore
		p:   {x:   1},
			q:   U
	): void;

	// so does an abstract method signature
	abstract class H {
		abstract m(
			// prettier-ignore
			p:   {x:   1},
			q:   U
		): void;
	}

	// a lone huggable parameter expands instead — a hug would pull the directive off its line
	type I = (
		// prettier-ignore
		p:   {x:   1}
	) => void;

	// a multi-line frozen parameter keeps its verbatim layout
	type J = (
		p:   T,
		// prettier-ignore
		q: {
			x:   1,
				y: 2
		},
			r:   U
	) => void;
</script>
