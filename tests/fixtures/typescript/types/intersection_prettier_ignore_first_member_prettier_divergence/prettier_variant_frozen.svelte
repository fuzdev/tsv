<script lang="ts">
	// an own-line directive before an intersection freezes only the FIRST member (Rule A:
	// the first member behaves like every honored list position); the separators are
	// parent-owned and the later members reformat normally. prettier instead freezes the
	// WHOLE intersection verbatim (see prettier_variant_frozen) — it has no intersection
	// printer, so its ignore handling is an unmaintained emergent passthrough
	type A =
		// prettier-ignore
		{a:1}  &  {b:2} & {c:3};

	// non-object members: freezing the first member is a visual no-op, the rest reformat
	type B =
		// prettier-ignore
		a1&a2&c1;

	// a leading & normalizes away on a multi-member intersection (the first member is a2's
	// sibling, not a one-element wrapper); prettier keeps the authored leading & verbatim
	type C =
		// prettier-ignore
		& a1&a2;
</script>
