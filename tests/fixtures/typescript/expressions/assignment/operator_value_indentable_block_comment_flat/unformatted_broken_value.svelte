<script lang="ts">
	// An indentable block comment leading the value hangs the value under the operator
	// (its `_hang` sibling), but it must not BREAK the value's own group: prettier prints
	// a leading comment outside that group, so a value that fits stays flat below the
	// comment. Every value kind here carries a group of its own.

	// binary chain
	const a =
		/**
		 * c
		 */ b +
		c +
		d;

	// ternary
	const e =
		/**
		 * c
		 */ f
			? g
			: h;

	// member chain
	const i =
		/**
		 * c
		 */ j
			.k()
			.l()
			.m();

	// assignment expression
	let n;
	n =
		/**
		 * c
		 */ o +
		p +
		q;

	// object property
	const r = {
		s:
			/**
			 * c
			 */ t +
			u +
			v
	};

	// class field
	class W {
		x =
			/**
			 * c
			 */ y +
			z +
			a1;
	}

	// type alias: a union must not gain a fabricated leading `|`
	type B1 =
		/**
		 * c
		 */ | C1
		| D1;

	// null control: a value with no group of its own
	const e1 =
		/**
		 * c
		 */ f1;

	// null control: a paren the printer keeps
	const g1 =
		/**
		 * c
		 */ (h1, i1);
</script>
