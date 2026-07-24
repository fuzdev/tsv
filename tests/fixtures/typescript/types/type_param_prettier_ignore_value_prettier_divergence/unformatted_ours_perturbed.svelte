<script lang="ts">
	// an own-line directive before a constraint stays own-line and freezes it
	type A<
		T extends
				// prettier-ignore
				{x:   1}
	> = T;

	// function host
	function fn<
		T extends
			// prettier-ignore
			{x:   1}
	>(x: T) {}

	// own-line before a default value
	type B<
		T  =
			// prettier-ignore
			{x:   1}
	> = T;

	// trailing the `extends` head = inert: the constraint formats normally
	type C<
		T extends // prettier-ignore
			{x:1}
	> = T;

	// union constraint: Rule A — the first member freezes
	type D<
		T extends
			// prettier-ignore
			{x:   1} | c1
	> = T;
</script>
