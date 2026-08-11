<script lang="ts">
	// A multiline block the author broke after keeps the break: the body `{` drops
	// to its own line, flush with the head. Both formatters agree here.
	let a = function () /* x
y */
	{
		fn();
	};

	// Object method - same gap, same agreement.
	let b = {
		c() /* x
y */
		{
			fn();
		}
	};

	// One whose `{` shares the comment's closing line stays glued - the
	// not-broke-after form, kept by both.
	let d = function () /* x
y */ {
		fn();
	};

	// A single-line block forces nothing, so its breaks collapse either way.
	let e = function () /* c */ {
		fn();
	};
</script>
