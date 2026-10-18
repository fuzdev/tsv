<script lang="ts">
	// type parameters - 100 chars to the `&`: the first member stays on the `=` line
	type A<T extends U> = XXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX<T> & // c
		Y;

	// type parameters - 101 chars to the `&`: the intersection breaks after `=`
	type B<T extends U> =
		XXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX<T> & // c
			Y;

	// no type parameters - 100 chars to the `&`
	type C = XXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX<T> & // c
		Y;

	// no type parameters - 101 chars to the `&`
	type D =
		XXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX<T> & // c
			Y;
</script>
