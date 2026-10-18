<script lang="ts">
	// A single type argument that is one object-like type with a type-argument list that can break,
	// united only with `null` / `void`, keeps the whole union on one line in an assignment or a type
	// alias: the line breaks after the `=`, a call's arguments free to break below, and only when the
	// line after the `=` is still over print width up to the value's first break (a call's `(`) does
	// it break inside the outer `<>`. Prettier breaks inside the inner `Map<>` instead.

	// 100 chars - stays inline
	const [aaaaaaaaaa01, setAaaaaaaaaa01] = fn<Map<Kkkkkkkkkk, Vvvvvvvvvvvvvvvvvvvvvvv> | null>(null);

	// the `(` at 100 chars - the arguments break (both us and Prettier)
	const [aaaaaaaaaa02, setAaaaaaaaaa02] = fn<Map<Kkkkkkkkkk, Vvvvvvvvvvvvvvvvvvvvvvvvvvvvv> | null>(
		null
	);

	// the `(` at 101 chars - breaks after the `=`, the type argument whole (Prettier breaks inside
	// the inner `Map<>`)
	const [aaaaaaaaaa03, setAaaaaaaaaa03] =
		fn<Map<Kkkkkkkkkk, Vvvvvvvvvvvvvvvvvvvvvvvvvvvvvv> | null>(null);

	// the line after the `=` at 100 chars - still breaks after the `=`
	const [aaaaaaaaaa04, setAaaaaaaaaa04] =
		fn<Map<Kkkkkkkkkk, Vvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvv> | null>(null);

	// the line after the `=` at 101 chars - the `(` still fits after the `=`, so the arguments break
	const [aaaaaaaaaa05, setAaaaaaaaaa05] =
		fn<Map<Kkkkkkkkkk, Vvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvv> | null>(
			null
		);

	// the `(` after the `=` at 100 chars - still breaks after the `=`
	const [aaaaaaaaaa06, setAaaaaaaaaa06] =
		fn<Map<Kkkkkkkkkk, Vvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvv> | null>(
			null
		);

	// the `(` after the `=` at 101 chars - breaks inside the outer `<>` instead, the union whole
	const [aaaaaaaaaa07, setAaaaaaaaaa07] = fn<
		Map<Kkkkkkkkkk, Vvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvv> | null
	>(null);

	// a `new` expression: 100 chars - stays inline
	const aaaaaaaaaaaaaaaaaaaaaaaaa08 = new Bbbbbbbbbb<Map<Kkkkkkkkkk, Vvvvvvvvvvvvvvvvvvv> | null>();

	// 101 chars - breaks after the `=`
	const aaaaaaaaaaaaaaaaaaaaaaaaa09 =
		new Bbbbbbbbbb<Map<Kkkkkkkkkk, Vvvvvvvvvvvvvvvvvvvv> | null>();

	// the line after the `=` at 100 chars
	const aaaaaaaaaaaaaaaaaaaaaaaaa10 =
		new Bbbbbbbbbb<Map<Kkkkkkkkkk, Vvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvv> | null>();

	// the line after the `=` at 101 chars - breaks inside the outer `<>`
	const aaaaaaaaaaaaaaaaaaaaaaaaa11 = new Bbbbbbbbbb<
		Map<Kkkkkkkkkk, Vvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvv> | null
	>();

	// a type alias: 100 chars - stays inline
	type Aaaaaaaaaaaaaaaaaaaaaaaaa12 = Promise<Map<Kkkkkkkkkk, Vvvvvvvvvvvvvvvvvvvvvvvvvvvvv> | null>;

	// 101 chars - breaks after the `=`
	type Aaaaaaaaaaaaaaaaaaaaaaaaa13 =
		Promise<Map<Kkkkkkkkkk, Vvvvvvvvvvvvvvvvvvvvvvvvvvvvvv> | null>;

	// the line after the `=` at 100 chars
	type Aaaaaaaaaaaaaaaaaaaaaaaaa14 =
		Promise<Map<Kkkkkkkkkk, Vvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvv> | null>;

	// the line after the `=` at 101 chars - breaks inside the outer `<>`
	type Aaaaaaaaaaaaaaaaaaaaaaaaa15 = Promise<
		Map<Kkkkkkkkkk, Vvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvv> | null
	>;
</script>
