<script lang="ts">
	// A block comment the author broke after, ahead of a class property value too long to
	// sit behind it: the comment takes a line of its own and the value hangs below it.
	class C {
		// a call
		p /* c */ = fn(
			argumentNumberOne,
			argumentNumberTwo,
			argumentNumberThree,
			argumentNumberFour,
			argument5
		);

		// a curried chain too long for its line
		q /* c */ =
			(aaaaaaaaaaaaaaaaaaaaaaaaaaa) =>
			(bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb) =>
			(ccccccccccccccccccccc) =>
				d;

		// a binary chain
		r /* c */ =
			aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa +
			bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb +
			ccccccccccccccccccccc;
	}
</script>
