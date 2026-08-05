<script lang="ts">
	// Short: object last arg hugs, method body expands (well under 100)
	new Cls('a', {
		m1() {
			fn1();
		}
	});

	// Hug prefix exactly 100: object still hugs — its forced break doesn't push the head out
	new Cls('AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA', {
		m2() {
			fn2();
		}
	});

	// Hug prefix 101: head args no longer fit beside the object, all args expand
	new Cls(
		'AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA',
		{
			m3() {
				fn3();
			}
		}
	);

	// Same-shape plain call (control): hugs identically
	fn4('AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA', {
		m4() {
			fn5();
		}
	});
</script>
