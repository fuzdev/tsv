<script lang="ts">
	// A hugged sole destructured parameter breaks BEFORE a breakable return type:
	// the object pattern prints with no group of its own, so once the signature
	// overflows its properties expand while the return type stays inline.

	// Function: 100 chars stays inline, 101 expands the pattern
	function fn100({ aaaa, bbbb, cccc }: Ddddddddddddddddddddddddddddddddddddd): string | undefined {}
	function fn101({
		aaaa,
		bbbb,
		cccc
	}: Dddddddddddddddddddddddddddddddddddddd): string | undefined {}

	// a generic return type with a nullable union
	function fn({
		aaaa,
		bbbb,
		cccc
	}: Ddddddddddddddddddddddddddddddddddddddd): Promise<Eeee | null> {}

	// Bodyless declaration
	declare function decl({
		aaaa,
		bbbb,
		cccc
	}: Ddddddddddddddddddddddddddddddddd): string | undefined;

	// Arrow
	const arrow = ({
		aaaa,
		bbbb,
		cccc
	}: Ddddddddddddddddddddddddddddddddddd): string | undefined => {};

	// Class method, object method, and interface signature
	class C {
		method({
			aaaa,
			bbbb,
			cccc
		}: Dddddddddddddddddddddddddddddddddddddddddddd): string | undefined {}
	}
	const o = {
		async method({
			aaaa,
			bbbb,
			cccc
		}: Dddddddddddddddddddddddddddddddddddd): Promise<Eeee | null> {}
	};
	interface I {
		method({
			aaaa,
			bbbb,
			cccc
		}: Dddddddddddddddddddddddddddddddddddddddddddddd): string | undefined;
	}

	// Function type
	type A = ({
		aaaa,
		bbbb,
		cccc
	}: Ddddddddddddddddddddddddddddddddddddddddddd) => string | undefined;

	// An untyped pattern
	function untyped({
		aaaa,
		bbbb,
		cccc,
		dddd,
		eeee,
		ffff,
		gggg,
		hhhh,
		iiiiii
	}): string | undefined {}

	// A pattern annotated with an object type: both expand
	function literal({
		aaaa,
		bbbb
	}: {
		aaaa: Aaaaaaaaaaaaaaaaaaaaaaa;
		bbbb: B;
	}): string | undefined {}

	// A hugged array pattern keeps its own group: the return type breaks
	function array([aaaa, bbbb, cccc]: Dddddddddddddddddddddddddddddddddddddddd):
		string | undefined {}

	// A defaulted pattern keeps its own group: the return type breaks
	function defaulted({ aaaa, bbbb, cccc } = {}): Promise<
		Eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee | Ffff
	> {}

	// Not hugged: two parameters break one per line
	function two(
		{ aaaa, bbbb, cccc }: Dddddddddddddddddddddddddddddddddd,
		e: E
	): string | undefined {}

	// A return type with no break point: the pattern expands
	function plain({
		aaaa,
		bbbb,
		cccc
	}: Dddddddddddddddddddddddddddddddddddddddddddddddddd): string {}

	// A callback argument: the arguments break out and the pattern stays inline
	fn(
		({ aaaa, bbbb, cccc }: Ddddddddddddddddddddddddddddddddddddddddddddddd): string | undefined => {
			g(aaaa);
		}
	);
	fn(({ aaaa, bbbb, cccc }: Ddddddddddddddddddddddddddddddddddddddddddddddd): string | undefined =>
		g(aaaa)
	);
</script>
