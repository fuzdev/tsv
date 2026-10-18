<script lang="ts">
	// A single type argument that is one object-like type (a type reference or an
	// object type) united only with `null` / `void` does not break inside the `<>`:
	// when the call's `(` fits the arguments break, else the line breaks after the `=`.

	// 100 chars - stays inline
	const [aaaaaaaaaaaaaaaaaaaaa01, setAaaaaaaaaaaaaaaaaaaaa01] = fn<Bbbbbbbbbbbbbbbbbb | null>(null);

	// the `(` at 100 chars - the arguments break
	const [aaaaaaaaaaaaaaaaaaaaa02, setAaaaaaaaaaaaaaaaaaaaa02] = fn<Bbbbbbbbbbbbbbbbbbbbbbbb | null>(
		null
	);

	// the `(` at 101 chars - breaks after the `=`
	const [aaaaaaaaaaaaaaaaaaaaa03, setAaaaaaaaaaaaaaaaaaaaa03] =
		fn<Bbbbbbbbbbbbbbbbbbbbbbbbb | null>(null);
	const aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa04 =
		fn<Bbbbbbbbbbbbbbbbbbbbb | null>(null);

	// the members in any order, `void` like `null`, and a reference with its own type arguments
	const [aaaaaaaaaaaaaaaaaaaaa05, setAaaaaaaaaaaaaaaaaaaaa05] =
		fn<null | Bbbbbbbbbbbbbbbbbbbbbbbbb>(null);
	const [aaaaaaaaaaaaaaaaaaaaa06, setAaaaaaaaaaaaaaaaaaaaa06] =
		fn<Bbbbbbbbbbbbbbbbbbbbbbbbb | void>(null);
	const [aaaaaaaaaaaaaaaaaaaaa07, setAaaaaaaaaaaaaaaaaaaaa07] =
		fn<Bbbbbbbbbbbbbbbbbb | null | void>(null);
	const [aaaaaaaaaaaaaaaaaaaaa08, setAaaaaaaaaaaaaaaaaaaaa08] =
		fn<Bbbbbbbbbbbbbbbbbbbbbb<C> | null>(null);

	// a `new` expression
	const aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa09 =
		new Bbbbbbbbbbbbbbb<Cccccc | null>();

	// a type alias: 100 chars stays inline, 101 breaks after the `=`
	type Aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa01 = Promise<Bbbbbbbbbbbbbbbbbbbbbbbbbbbbb | null>;
	type Aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa02 =
		Promise<Bbbbbbbbbbbbbbbbbbbbbbbbbbbbbb | null>;
	type Aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa03 =
		Promise<Bbbbbbbbbbbbbbbbbbbbbbbbbbbbbb | void>;

	// an arrow's return type, in an assignment
	const aaaaaaaaaaaaaaaaaaaaaaa10 =
		(): Promise<Bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb | void> => {};

	// a return type after a parameter list that can break: the parameters break
	function ffff({
		aaaa,
		bbbb,
		cccc
	} = {}): Promise<Eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee | null> {}

	// a simple type argument takes the same layout
	const [aaaaaaaaaaaaaaaaaaaaa11, setAaaaaaaaaaaaaaaaaaaaa11] =
		fn<Bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb>(null);

	// an object type united with `null` hugs the `<`
	const [aaaaaaaaaaaaaaaaaaaaa12, setAaaaaaaaaaaaaaaaaaaaa12] = fn<{
		bbbbbbbbbbbbbbbbbb: C;
	} | null>(null);

	// `undefined`, a second object-like member, or a second type argument break inside the `<>`
	const [aaaaaaaaaaaaaaaaaaaaa13, setAaaaaaaaaaaaaaaaaaaaa13] = fn<
		Bbbbbbbbbbbbbbbbbbbb | undefined
	>(null);
	const [aaaaaaaaaaaaaaaaaaaaa14, setAaaaaaaaaaaaaaaaaaaaa14] = fn<
		Bbbbbbbbbbbbbbbbbbbbbbbbb | Cccc
	>(null);
	const [aaaaaaaaaaaaaaaaaaaaa15, setAaaaaaaaaaaaaaaaaaaaa15] = fn<
		Bbbbbbbbbbbbbbbbbbbbbb | null,
		C
	>(null);
</script>
