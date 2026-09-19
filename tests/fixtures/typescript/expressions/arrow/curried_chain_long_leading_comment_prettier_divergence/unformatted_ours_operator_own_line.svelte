<script lang="ts">
	// A curried chain too long for its line breaks after the `=` and stacks its heads at one
	// shared indent. A comment on the `=` line, or glued to the first head, does not change that.

	// null control: no comment
	const a =
		(aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa) =>
		(bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb) =>
		(cccccccccccccccccccccccccccccccccc) => {
			return d;
		};

	// line comment
	const b = // c
		(aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa) =>
		(bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb) =>
		(cccccccccccccccccccccccccccccccccc) => {
			return d;
		};

	// single-line block glued to the first head
	const c =
		/* c */ (aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa) =>
		(bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb) =>
		(cccccccccccccccccccccccccccccccccc) => {
			return d;
		};

	// own-line block: a run the author gave a line of its own above the chain is prettier's
	// leading own-line comment, whose chain takes the default shape — the heads past the first
	// indented under it
	const e =
		/* c */
		(aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa) =>
			(bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb) =>
			(cccccccccccccccccccccccccccccccccc) => {
				return d;
			};

	// preserved multi-line block: its text spans lines itself, so it cannot ride behind the
	// chain's break — it leads the chain from the `=` line, the first head on its closing
	// line and the rest indented under it
	const f = /* x
y */ (aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa) =>
		(bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb) =>
		(cccccccccccccccccccccccccccccccccc) => {
			return d;
		};

	// indentable block
	const g =
		/**
		 * x
		 */ (aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa) =>
		(bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb) =>
		(cccccccccccccccccccccccccccccccccc) => {
			return d;
		};

	// an assignment, a class property and an object property take the same shape
	x.y

		= // c
		(aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa) =>
		(bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb) =>
		(cccccccccccccccccccccccccccccccccc) => {
			return d;
		};

	class K {
		p = // c
			(aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa) =>
			(bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb) =>
			(cccccccccccccccccccccccccccccccccc) => {
				return d;
			};
	}

	const o = {
		k: // c
			(aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa) =>
			(bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb) =>
			(cccccccccccccccccccccccccccccccccc) => {
				return d;
			}
	};

	// the own-line run at an assignment and a class property: the same default shape, and no
	// blank line below the comment
	x.z =
		/* c */
		(aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa) =>
			(bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb) =>
			(cccccccccccccccccccccccccccccccccc) => {
				return d;
			};

	class L {
		p =
			/* c */
			(aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa) =>
				(bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb) =>
				(cccccccccccccccccccccccccccccccccc) => {
					return d;
				};
	}
</script>
