<script lang="ts">
	// A real directive prologue stays bare.
	'use strict';

	// A later bare string is not part of the prologue.
	let a;
	(('foo'));

	// A leading string in a plain block.
	{
		(('bar'));
	}

	// A non-leading string inside a function body.
	function fn() {
		let b;
		(('baz'));
	}

	// A leading string in a catch block.
	try {
		fn1();
	} catch (e) {
		(('qux'));
		fn2();
	}

	// A string inside a static block stays bare (not a Program/BlockStatement position).
	class A {
		static {
			(('quux'));
		}
	}

	// A string in a switch case body also stays bare.
	switch (x) {
		case 1:
			(('corge'));
			break;
	}

	// A string in a namespace body also stays bare.
	namespace N {
		(('grault'));
		const c = 1;
	}
</script>
