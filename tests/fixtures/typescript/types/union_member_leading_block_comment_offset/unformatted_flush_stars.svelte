<script lang="ts">
	// Prettier prints a union member's LEADING comments INSIDE that member's own
	// `align(2)` offset, so an indentable block's reindented continuation lines sit two
	// columns past the `|`. A comment TRAILING the previous member prints OUTSIDE the
	// offset and stays flush under the `|` — the pair is what makes the offset a
	// property of the leading position, not of the union.

	// Leading, second member.
	type A =
		| Aaaa
		| /**
		* c
		*/ Bbbb;

	// Leading, third member: the same offset wherever the member sits.
	type B =
		| Aaaa
		| Bbbb
		| /**
		* c
		*/ Cccc;

	// Trailing the previous member: no offset.
	type C =
		| Aaaa /**
		 * c
		 */
		| Bbbb;

	// A single-line block opens no line of its own, so the offset is inert.
	type D = Aaaa | /* c */ Bbbb;

	// A non-indentable block renders verbatim — nothing reindents, so nothing offsets.
	type E =
		| Aaaa
		| /* c
d */ Bbbb;

	// An intersection has no per-member offset at all.
	type F = Aaaa &
		Bbbb &
		/**
				 * c
				 */ Cccc;

	// A `//` anywhere in the union switches on the line-comment layout, which emits an
	// inline leading run through a builder of its own. Same rule, same offset.
	type G =
		| Aaaa // x
		| Bbbb
		| /**
		* c
		*/ Cccc;

	type H =
		| Aaaa
		| /**
		* c
		*/ Bbbb // x
		| Cccc;
</script>
