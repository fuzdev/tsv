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

	// Dynamic import — the same rule, its fourth spelling
	import(  `line 1
line 2
line 3
`  );

	// Control: a second argument defeats the hug (the rule is sole-argument only)
	import(  `line 1
line 2
line 3
`  ,  opts  );

	// Member chain — the rule outranks chain routing, so the heads stay flat
	a.b().c().d(  `line 1
line 2
line 3
`  );
</script>
