<script lang="ts">
	// An intersection member whose OWN type arguments break inside `<...>`, in a
	// hanging-indent position. The first member sits at base indent (its `<...>`
	// args indent one level, its `>` returns to base); only the trailing `& Foo`
	// continuation indents one level.

	// Type-alias RHS
	type Alias = Aaaaaaaaaaaaaaaaaaaaaaaaaaaaaa<Bbbbbbbbbbbbbbbbbb, Cccccccccccccccccc, Dddddddddddddddddd, Eeeeeeeeeeeeeeeeee> & Foo;

	// Function-type return
	type Fn = () => Aaaaaaaaaaaaaaaaaaaaaaaaaaaaaa<Bbbbbbbbbbbbbbbbbb, Cccccccccccccccccc, Dddddddddddddddddd, Eeeeeeeeeeeeeeeeee> & Foo;

	// `as` cast
	const x = y as Aaaaaaaaaaaaaaaaaaaaaaaaaaaaaa<Bbbbbbbbbbbbbbbbbb, Cccccccccccccccccc, Dddddddddddddddddd, Eeeeeeeeeeeeeeeeee> & Foo;
</script>
