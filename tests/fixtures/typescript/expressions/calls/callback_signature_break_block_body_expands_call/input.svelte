<script lang="ts">
	// a parameter's authored-expanded object type, beside another parameter, breaks every
	// argument out around a block body
	on(
		x,
		(
			b: any,
			a: {
				c: string;
			}
		) => {
			foo();
		}
	);

	// a lone argument
	on(
		(
			b: any,
			a: {
				c: string;
			}
		) => {
			foo();
		}
	);

	// new
	new On(
		(
			b: any,
			a: {
				c: string;
			}
		) => {
			foo();
		}
	);

	// member chain
	obj.on(
		x,
		(
			b: any,
			a: {
				c: string;
			}
		) => {
			foo();
		}
	);

	// an authored-expanded default object breaks every argument out around a block body
	fn(
		(
			a = {
				b: 1
			}
		) => {
			g(a);
		}
	);

	// a preceding argument
	fn(
		lead,
		(
			a = {
				b: 1
			}
		) => {
			g(a);
		}
	);

	// an authored-expanded object return type
	on(
		x,
		(
			b: any,
			a: A
		): {
			c: string;
		} => {
			foo();
		}
	);

	// a lone argument
	memoize(
		(
			a: A
		): {
			c: string;
		} => {
			foo();
		}
	);

	// a curried callee
	memoize(1)(
		(
			a: A
		): {
			c: string;
		} => {
			foo();
		}
	);

	// new
	new Memoize(
		(
			a: A
		): {
			c: string;
		} => {
			foo();
		}
	);

	// member chain
	obj.memoize(
		(
			a: A
		): {
			c: string;
		} => {
			foo();
		}
	);

	// a forced break no newline spells breaks every argument out too: a matrix default array
	fn(
		lead,
		(
			a = [
				[1, 2],
				[3, 4]
			]
		) => {
			g(a);
		}
	);

	// a lone argument
	fn(
		(
			a = [
				[1, 2],
				[3, 4]
			]
		) => {
			g(a);
		}
	);

	// a member chain the chain printer breaks
	fn(
		lead,
		(
			a = x
				.a(() => 1)
				.b(() => 2)
				.c(() => 3)
				.d(() => 4)
		) => {
			g(a);
		}
	);

	// a curried callee
	fn(a)(
		lead,
		(
			a = {
				b: 1
			}
		) => {
			g(a);
		}
	);

	// a curried callee, a function
	fn(a)(
		lead,
		function (
			a = {
				b: 1
			}
		) {
			g(a);
		}
	);

	// a break inside a sole destructured parameter: a default object
	fn(
		lead,
		({
			a = {
				b: 1
			}
		}) => {
			g(a);
		}
	);

	// a nested pattern
	fn(
		lead,
		({
			a: {
				b: { c }
			}
		}) => {
			g(a);
		}
	);

	// a blank line between properties
	fn(
		lead,
		({
			a,

			b
		}) => {
			g(a);
		}
	);

	// a nested object type the author expanded, inside a sole parameter's object type
	fn(
		lead,
		(a: {
			b: {
				c: D;
			};
		}) => {
			g(a);
		}
	);

	// inside the object type annotating a sole destructured parameter
	fn(
		lead,
		({
			a
		}: {
			a: {
				c: D;
			};
		}) => {
			g(a);
		}
	);

	// a blank line between the members of a sole parameter's object type
	fn(
		lead,
		(a: {
			b: C;

			d: E;
		}) => {
			g(a);
		}
	);

	// a lone argument
	fn(
		(a: {
			b: C;

			d: E;
		}) => {
			g(a);
		}
	);

	// expand-first
	fn(
		(a: {
			b: C;

			d: E;
		}) => {
			g(a);
		},
		lead
	);

	// annotating a sole destructured parameter
	fn(
		lead,
		({
			a
		}: {
			a: C;

			d: E;
		}) => {
			g(a);
		}
	);

	// a function's sole parameter
	fn(
		lead,
		function (a: {
			b: C;

			d: E;
		}) {
			g(a);
		}
	);

	// control: a declaration keeps its parameter hugged
	const f = (a: {
		b: C;

		d: E;
	}) => {
		g(a);
	};

	// a function's sole destructured parameter
	fn(
		lead,
		function ({
			a = {
				b: 1
			}
		}) {
			g(a);
		}
	);

	// a function after a preceding argument
	on(
		x,
		function (
			b: any,
			a: {
				c: string;
			}
		) {
			foo();
		}
	);

	// control: a lone function keeps the hug, its parameters broken
	on(function (
		b: any,
		a: {
			c: string;
		}
	) {
		foo();
	});

	// control: so does a function after a preceding argument of new
	new On(x, function (
		b: any,
		a: {
			c: string;
		}
	) {
		foo();
	});
</script>
