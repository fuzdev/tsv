<script lang="ts">
	// A sole destructured parameter hugs its parens, and a hugged pattern stays inline
	// however it nests — in a signature with no body as in a function with one.
	type F1 = ({ a: { b } }: T) => U;
	type F2 = ({ a: [b] }: T) => U;
	type F3 = ({ a: { b }, c }: T) => U;
	type C1 = new ({ a: { b } }: T) => U;
	type C2 = abstract new ({ a: { b } }: T) => U;
	declare function fn1({ a: { b } }: T): U;
	function fn2({ a: { b } }: T): U;
	function fn2({ a: { b } }: T): U {}
	type L = {
		m({ a: { b } }: T): U;
		({ a: { b } }: T): U;
		new ({ a: { b } }: T): U;
	};
	interface I {
		m({ a: { b } }: T): U;
		({ a: { b } }: T): U;
		new ({ a: { b } }: T): U;
	}
	declare class D {
		constructor({ a: { b } }: T);
		m({ a: { b } }: T): U;
	}
	abstract class E {
		abstract m({ a: { b } }: T): U;
	}

	// A third level expands the hugged pattern, as it does in a function with a body.
	type F5 = ({
		a: {
			b: { c }
		}
	}: T) => U;
	declare function fn5({
		a: {
			b: { c }
		}
	}: T): U;
	function fn6({
		a: {
			b: { c }
		}
	}: T): U {}
	interface J {
		m({
			a: {
				b: { c }
			}
		}: T): U;
	}

	// An optional pattern hugs the same way. A rest parameter does not hug, and an array
	// pattern's element is no parameter, so the pattern inside either is an ordinary one.
	type F6 = ({ a: { b } }?: T) => U;
	type F7 = (
		...{
			a: { b }
		}: T
	) => U;
	type F8 = ([
		{
			a: { b }
		}
	]: T) => U;

	// Beside a second parameter nothing hugs, and the nested pattern breaks where no
	// function body follows.
	type F4 = (
		{
			a: { b }
		}: T,
		c: V
	) => U;
	declare function fn3(
		{
			a: { b }
		}: T,
		c: V
	): U;
	function fn4({ a: { b } }: T, c: V): U {}
</script>
