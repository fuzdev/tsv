<script lang="ts">
	// Multiline template literal as single call arg — hugged form.
	// When the template starts on the same line as (, prettier hugs it.

	// Simple callee
	fn(   `line 1
line 2
line 3
`  );

	// Member expression callee
	a.push(  `line 1
line 2
line 3
`  );

	// With interpolation
	console.log(  `text ${a}
more text ${b}
`  );

	// Short control (single line — no multiline)
	fn(  `short`  );
</script>
