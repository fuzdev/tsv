<script lang="ts">
	// The expand-first hug keeps a short tail argument inline past the callback's closing
	// brace, so that closing line is what print width measures. At 100 it holds and the
	// tail's operands stay flat in all three argument printers.
	fn(() => {
		a();
	}, aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa && bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb);

	new A(() => {
		a();
	}, aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa && bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb);

	obj.fn1(1).fn2(() => {
		a();
	}, aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa && bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb);

	// At 101 tsv breaks the tail's operator and indents the continuation; prettier holds
	// the tail flat at any width.
	fn(() => {
		a();
	}, aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa && bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb);

	new A(() => {
		a();
	}, aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa && bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb);

	obj.fn1(1).fn2(() => {
		a();
	}, aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa && bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb);
</script>
