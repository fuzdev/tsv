<script lang="ts">
	// an authored-expanded default object breaks every argument out around an object body
	fn(
		(
			a = {
				b: 1
			}
		) => ({ a })
	);

	// new
	new Xy(
		(
			a = {
				b: 1
			}
		) => ({ a })
	);

	// member chain
	obj.a.fn(
		(
			a = {
				b: 1
			}
		) => ({ a })
	);

	// a preceding argument
	fn(
		lead,
		(
			a = {
				b: 1
			}
		) => ({ a })
	);

	// a preceding argument, member chain
	obj.a.fn(
		lead,
		(
			a = {
				b: 1
			}
		) => ({ a })
	);

	// an authored-expanded default object breaks every argument out around an array body
	fn(
		(
			a = {
				b: 1
			}
		) => [a]
	);

	// new
	new Xy(
		(
			a = {
				b: 1
			}
		) => [a]
	);

	// member chain
	obj.a.fn(
		(
			a = {
				b: 1
			}
		) => [a]
	);

	// a preceding argument
	fn(
		lead,
		(
			a = {
				b: 1
			}
		) => [a]
	);

	// a preceding argument, member chain
	obj.a.fn(
		lead,
		(
			a = {
				b: 1
			}
		) => [a]
	);

	// an authored-expanded default object breaks every argument out around a conditional body
	fn(
		(
			a = {
				b: 1
			}
		) => (a ? b : c)
	);

	// new
	new Xy(
		(
			a = {
				b: 1
			}
		) => (a ? b : c)
	);

	// member chain
	obj.a.fn(
		(
			a = {
				b: 1
			}
		) => (a ? b : c)
	);

	// a preceding argument
	fn(
		lead,
		(
			a = {
				b: 1
			}
		) => (a ? b : c)
	);

	// a preceding argument, member chain
	obj.a.fn(
		lead,
		(
			a = {
				b: 1
			}
		) => (a ? b : c)
	);

	// a default object on a later parameter
	fn(
		(
			a,
			b = {
				b: 1
			}
		) => (a ? b : c)
	);

	// a later parameter, member chain
	obj.a.fn(
		(
			a,
			b = {
				b: 1
			}
		) => (a ? b : c)
	);

	// an async arrow, new
	new Xy(
		async (
			a = {
				b: 1
			}
		) => ({ a })
	);

	// an async arrow, member chain
	obj.a.fn(
		async (
			a = {
				b: 1
			}
		) => (a ? b : c)
	);

	// a return type
	fn(
		(
			a = {
				b: 1
			}
		): T => [a]
	);

	// a return type, member chain
	obj.a.fn(
		(
			a = {
				b: 1
			}
		): T => (a ? b : c)
	);

	// a destructured parameter's default
	fn(
		(
			{ a } = {
				b: 1
			}
		) => ({ a })
	);

	// a forced break no newline spells breaks every argument out too: a matrix default array,
	// around a call body
	fn(
		lead,
		(
			a = [
				[1, 2],
				[3, 4]
			]
		) => g(a)
	);

	// a lone argument
	fn(
		(
			a = [
				[1, 2],
				[3, 4]
			]
		) => g(a)
	);

	// new
	new Fn(
		lead,
		(
			a = [
				[1, 2],
				[3, 4]
			]
		) => g(a)
	);

	// new, a lone argument
	new Fn(
		(
			a = [
				[1, 2],
				[3, 4]
			]
		) => g(a)
	);

	// member chain
	obj.a.fn(
		lead,
		(
			a = [
				[1, 2],
				[3, 4]
			]
		) => g(a)
	);

	// member chain, a lone argument
	obj.a.fn(
		(
			a = [
				[1, 2],
				[3, 4]
			]
		) => g(a)
	);

	// an optional call
	fn?.(
		lead,
		(
			a = [
				[1, 2],
				[3, 4]
			]
		) => g(a)
	);

	// a curried callee
	fn(a)(
		lead,
		(
			a = [
				[1, 2],
				[3, 4]
			]
		) => g(a)
	);

	// a tagged-template callee
	tag`t`(
		lead,
		(
			a = [
				[1, 2],
				[3, 4]
			]
		) => g(a)
	);

	// an async arrow
	fn(
		lead,
		async (
			a = [
				[1, 2],
				[3, 4]
			]
		) => g(a)
	);

	// a lone argument
	fn(
		async (
			a = [
				[1, 2],
				[3, 4]
			]
		) => g(a)
	);

	// new
	new Fn(
		lead,
		async (
			a = [
				[1, 2],
				[3, 4]
			]
		) => g(a)
	);

	// new, a lone argument
	new Fn(
		async (
			a = [
				[1, 2],
				[3, 4]
			]
		) => g(a)
	);

	// member chain
	obj.a.fn(
		lead,
		async (
			a = [
				[1, 2],
				[3, 4]
			]
		) => g(a)
	);

	// member chain, a lone argument
	obj.a.fn(
		async (
			a = [
				[1, 2],
				[3, 4]
			]
		) => g(a)
	);

	// an optional call
	fn?.(
		lead,
		async (
			a = [
				[1, 2],
				[3, 4]
			]
		) => g(a)
	);

	// a curried callee
	fn(a)(
		lead,
		async (
			a = [
				[1, 2],
				[3, 4]
			]
		) => g(a)
	);

	// a tagged-template callee
	tag`t`(
		lead,
		async (
			a = [
				[1, 2],
				[3, 4]
			]
		) => g(a)
	);

	// a matrix default array around a conditional body
	fn(
		lead,
		(
			a = [
				[1, 2],
				[3, 4]
			]
		) => (a ? b : c)
	);

	// new
	new Fn(
		lead,
		(
			a = [
				[1, 2],
				[3, 4]
			]
		) => (a ? b : c)
	);

	// member chain
	obj.a.fn(
		lead,
		(
			a = [
				[1, 2],
				[3, 4]
			]
		) => (a ? b : c)
	);

	// an optional call
	fn?.(
		lead,
		(
			a = [
				[1, 2],
				[3, 4]
			]
		) => (a ? b : c)
	);

	// a curried callee
	fn(a)(
		lead,
		(
			a = [
				[1, 2],
				[3, 4]
			]
		) => (a ? b : c)
	);

	// a tagged-template callee
	tag`t`(
		lead,
		(
			a = [
				[1, 2],
				[3, 4]
			]
		) => (a ? b : c)
	);

	// a matrix inside a call in the default
	fn(
		lead,
		(
			a = new Map([
				[1, 2],
				[3, 4]
			])
		) => g(a)
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
		) => g(a)
	);

	// a blank line between the members of a sole parameter's object type, around a call body
	fn(
		lead,
		(a: {
			b: C;

			d: E;
		}) => g(a)
	);

	// expand-first
	fn(
		(a: {
			b: C;

			d: E;
		}) => g(a),
		lead
	);
</script>
